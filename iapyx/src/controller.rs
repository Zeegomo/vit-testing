use crate::SimpleVoteStatus;
use crate::Wallet;
use crate::{data::Proposal as VitProposal, WalletBackend};
use bech32::FromBase32;
use bip39::Type;
use chain_impl_mockchain::{fragment::FragmentId, transaction::Input};
use jormungandr_lib::interfaces::{AccountState, FragmentLog, FragmentStatus};
use jormungandr_testing_utils::qr_code::KeyQrCode;
use jormungandr_testing_utils::testing::node::RestSettings;
use std::collections::HashMap;
use std::convert::TryInto;
use std::path::Path;
use thiserror::Error;
use wallet::{AccountId, Settings};
use wallet_core::{Choice, Value};

pub struct Controller {
    backend: WalletBackend,
    wallet: Wallet,
    settings: Settings,
}

impl Controller {
    pub fn generate(
        proxy_address: String,
        words_length: Type,
        backend_settings: RestSettings,
    ) -> Result<Self, ControllerError> {
        let backend = WalletBackend::new(proxy_address, backend_settings);
        let settings = backend.settings()?;
        Ok(Self {
            backend,
            wallet: Wallet::generate(words_length)?,
            settings,
        })
    }

    pub fn recover_with_backend(
        backend: WalletBackend,
        mnemonics: &str,
        password: &[u8],
    ) -> Result<Self, ControllerError> {
        let settings = backend.settings()?;
        Ok(Self {
            backend,
            wallet: Wallet::recover(mnemonics, password)?,
            settings,
        })
    }

    pub fn recover(
        proxy_address: String,
        mnemonics: &str,
        password: &[u8],
        backend_settings: RestSettings,
    ) -> Result<Self, ControllerError> {
        let backend = WalletBackend::new(proxy_address, backend_settings);
        Self::recover_with_backend(backend, mnemonics, password)
    }

    pub fn recover_account(
        proxy_address: String,
        account: &[u8],
        backend_settings: RestSettings,
    ) -> Result<Self, ControllerError> {
        let backend = WalletBackend::new(proxy_address, backend_settings);
        let settings = backend.settings()?;
        Ok(Self {
            backend,
            wallet: Wallet::recover_from_account(account)?,
            settings,
        })
    }

    pub fn recover_from_qr<P: AsRef<Path>>(
        proxy_address: String,
        qr: P,
        password: &str,
        backend_settings: RestSettings,
    ) -> Result<Self, ControllerError> {
        let img = image::open(qr.as_ref())?;
        let bytes: Vec<u8> = password
            .chars()
            .map(|x| x.to_digit(10).unwrap() as u8)
            .collect();
        let secret = KeyQrCode::decode(img, &bytes)
            .unwrap()
            .get(0)
            .unwrap()
            .clone()
            .leak_secret();
        let backend = WalletBackend::new(proxy_address, backend_settings);
        let settings = backend.settings()?;
        Ok(Self {
            backend,
            wallet: Wallet::recover_from_utxo(secret.as_ref().try_into().unwrap())?,
            settings,
        })
    }

    pub fn recover_from_sk<P: AsRef<Path>>(
        proxy_address: String,
        private_key: P,
        backend_settings: RestSettings,
    ) -> Result<Self, ControllerError> {
        let (_, data) = read_bech32(private_key)?;
        let key_bytes = Vec::<u8>::from_base32(&data)?;
        let data: [u8; 64] = key_bytes.try_into().unwrap();
        let backend = WalletBackend::new(proxy_address, backend_settings);
        let settings = backend.settings()?;
        Ok(Self {
            backend,
            wallet: Wallet::recover_from_utxo(&data)?,
            settings,
        })
    }

    pub fn switch_backend(&mut self, proxy_address: String, backend_settings: RestSettings) {
        self.backend = WalletBackend::new(proxy_address, backend_settings);
    }

    pub fn account(&self, discrimination: chain_addr::Discrimination) -> chain_addr::Address {
        self.wallet.account(discrimination)
    }

    pub fn id(&self) -> AccountId {
        self.wallet.id()
    }

    pub fn send_fragment(&self, transaction: &[u8]) -> Result<FragmentId, ControllerError> {
        self.send_fragments(vec![transaction.to_vec()])
            .map(|v| *v.first().unwrap())
    }

    pub fn send_fragments(
        &self,
        transaction: Vec<Vec<u8>>,
    ) -> Result<Vec<FragmentId>, ControllerError> {
        self.backend.send_fragments(transaction).map_err(Into::into)
    }

    pub fn confirm_all_transactions(&mut self) {
        self.wallet.confirm_all_transactions();
    }

    pub fn confirm_transaction(&mut self, id: FragmentId) {
        self.wallet.confirm_transaction(id)
    }

    pub fn pending_transactions(&self) -> Vec<FragmentId> {
        self.wallet.pending_transactions()
    }

    pub fn wait_for_pending_transactions(
        &mut self,
        pace: std::time::Duration,
    ) -> Result<(), ControllerError> {
        let mut limit = 60;
        loop {
            let ids: Vec<FragmentId> = self.pending_transactions().to_vec();

            if limit <= 0 {
                return Err(ControllerError::TransactionsWerePendingForTooLong { fragments: ids });
            }

            if ids.is_empty() {
                return Ok(());
            }

            let fragment_logs = self.backend.fragment_logs().unwrap();
            for id in ids.iter() {
                if let Some(fragment) = fragment_logs.get(id) {
                    match fragment.status() {
                        FragmentStatus::Rejected { .. } => {
                            self.remove_pending_transaction(id);
                        }
                        FragmentStatus::InABlock { .. } => {
                            self.confirm_transaction(*id);
                        }
                        _ => (),
                    };
                }
            }

            if ids.is_empty() {
                return Ok(());
            } else {
                std::thread::sleep(pace);
                limit += 1;
            }
        }
    }

    pub fn remove_pending_transaction(&mut self, id: &FragmentId) -> Option<Vec<Input>> {
        self.wallet.remove_pending_transaction(id)
    }

    pub fn total_value(&self) -> Value {
        self.wallet.total_value()
    }

    pub fn refresh_state(&mut self) -> Result<(), ControllerError> {
        let account_state = self.get_account_state()?;
        let value: u64 = (*account_state.value()).into();
        self.wallet.set_state(Value(value), account_state.counter());
        Ok(())
    }

    pub fn get_account_state(&self) -> Result<AccountState, ControllerError> {
        self.backend.account_state(self.id()).map_err(Into::into)
    }

    pub fn vote_for(
        &mut self,
        vote_plan_id: String,
        proposal_index: u32,
        choice: u8,
    ) -> Result<FragmentId, ControllerError> {
        let proposals = self.get_proposals()?;
        let proposal = proposals
            .iter()
            .find(|x| {
                x.chain_voteplan_id == vote_plan_id
                    && x.chain_proposal_index == proposal_index as i64
            })
            .ok_or(ControllerError::CannotFindProposal {
                vote_plan_name: vote_plan_id.to_string(),
                proposal_index,
            })?;

        let transaction = self.wallet.vote(
            self.settings.clone(),
            &proposal.clone().into(),
            Choice::new(choice),
        )?;
        Ok(self.backend.send_fragment(transaction.to_vec())?)
    }

    pub fn vote(
        &mut self,
        proposal: &VitProposal,
        choice: Choice,
    ) -> Result<FragmentId, ControllerError> {
        let transaction =
            self.wallet
                .vote(self.settings.clone(), &proposal.clone().into(), choice)?;
        Ok(self.backend.send_fragment(transaction.to_vec())?)
    }

    pub fn get_proposals(&mut self) -> Result<Vec<VitProposal>, ControllerError> {
        Ok(self
            .backend
            .proposals()?
            .iter()
            .cloned()
            .map(Into::into)
            .collect())
    }

    pub fn fragment_logs(&self) -> Result<HashMap<FragmentId, FragmentLog>, ControllerError> {
        Ok(self.backend.fragment_logs()?)
    }

    pub fn active_votes(&self) -> Result<Vec<SimpleVoteStatus>, ControllerError> {
        Ok(self
            .backend
            .vote_statuses(self.wallet.identifier(self.settings.discrimination))?)
    }
}

pub fn read_bech32(path: impl AsRef<Path>) -> Result<(String, Vec<bech32::u5>), ControllerError> {
    let line = jortestkit::file::read_file(path);
    let line_without_special_characters = line.replace(&['\n', '\r'][..], "");
    bech32::decode(&line_without_special_characters).map_err(Into::into)
}

#[derive(Debug, Error)]
pub enum ControllerError {
    #[error("wallet error")]
    WalletError(#[from] crate::wallet::Error),
    #[error("backend error")]
    BackendError(#[from] crate::backend::WalletBackendError),
    #[error("cannot find proposal: voteplan({vote_plan_name}) index({proposal_index})")]
    CannotFindProposal {
        vote_plan_name: String,
        proposal_index: u32,
    },
    #[error("transactions with ids [{fragments:?}] were pending for too long")]
    TransactionsWerePendingForTooLong { fragments: Vec<FragmentId> },
    #[error("cannot read QR code from '{0}' path")]
    CannotReadQrCode(#[from] image::ImageError),
    #[error("bech32 error")]
    Bech32(#[from] bech32::Error),
}
