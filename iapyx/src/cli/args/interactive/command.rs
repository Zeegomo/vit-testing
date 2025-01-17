use super::WalletState;
use crate::cli::args::interactive::UserInteractionContoller;
use crate::Controller;
use bip39::Type;
use chain_addr::{AddressReadable, Discrimination};
use jormungandr_testing_utils::testing::node::RestSettings;
use std::path::PathBuf;
use structopt::{clap::AppSettings, StructOpt};
use thiserror::Error;
use wallet_core::Choice;

#[derive(StructOpt, Debug)]
#[structopt(setting = AppSettings::NoBinaryName)]
pub enum IapyxCommand {
    /// recover wallet funds from mnemonic
    Recover(Recover),
    /// generate new wallet
    Generate(Generate),
    /// connect to backend
    Connect(Connect),
    /// confirms transaction
    ConfirmTx,
    Value,
    /// Prints wallets, nodes which can be used. Draw topology
    Status,
    /// Prints wallets, nodes which can be used. Draw topology
    Refresh,
    /// get Address
    Address(Address),
    Logs,
    /// Exit interactive mode
    Exit,
    Proposals,
    Vote(Vote),
    Votes,
    PendingTransactions,
}

impl IapyxCommand {
    pub fn exec(&self, model: &mut UserInteractionContoller) -> Result<(), IapyxCommandError> {
        match self {
            IapyxCommand::PendingTransactions => {
                if let Some(controller) = model.controller.as_mut() {
                    let fragment_ids = controller
                        .pending_transactions()
                        .iter()
                        .map(|x| x.to_string())
                        .collect::<Vec<String>>();
                    println!("===================");
                    for (id, fragment_ids) in fragment_ids.iter().enumerate() {
                        println!("{}. {}", (id + 1), fragment_ids);
                    }
                    println!("===================");
                    return Ok(());
                }
                Err(IapyxCommandError::GeneralError(
                    "wallet not recovered or generated".to_string(),
                ))
            }
            IapyxCommand::Votes => {
                if let Some(controller) = model.controller.as_mut() {
                    println!("===================");
                    for (id, vote) in controller.active_votes()?.iter().enumerate() {
                        println!("{}. {}", (id + 1), vote);
                    }
                    println!("===================");
                    return Ok(());
                }
                Err(IapyxCommandError::GeneralError(
                    "wallet not recovered or generated".to_string(),
                ))
            }
            IapyxCommand::Proposals => {
                if let Some(controller) = model.controller.as_mut() {
                    println!("===================");
                    for (id, proposal) in controller.get_proposals()?.iter().enumerate() {
                        println!(
                            "{}. #{} [{}] {}",
                            (id + 1),
                            proposal.chain_proposal_id_as_str(),
                            proposal.proposal_title,
                            proposal.proposal_summary
                        );
                        println!("{:#?}", proposal.chain_vote_options.0);
                    }
                    println!("===================");
                    return Ok(());
                }
                Err(IapyxCommandError::GeneralError(
                    "wallet not recovered or generated".to_string(),
                ))
            }
            IapyxCommand::Vote(vote) => vote.exec(model),
            IapyxCommand::ConfirmTx => {
                if let Some(controller) = model.controller.as_mut() {
                    controller.confirm_all_transactions();
                    return Ok(());
                }
                Err(IapyxCommandError::GeneralError(
                    "wallet not recovered or generated".to_string(),
                ))
            }
            IapyxCommand::Recover(recover) => recover.exec(model),
            IapyxCommand::Exit => Ok(()),
            IapyxCommand::Generate(generate) => generate.exec(model),
            IapyxCommand::Connect(connect) => connect.exec(model),
            IapyxCommand::Value => {
                if let Some(controller) = model.controller.as_mut() {
                    println!("Total Value: {}", controller.total_value());
                    return Ok(());
                }
                Err(IapyxCommandError::GeneralError(
                    "wallet not recovered or generated".to_string(),
                ))
            }
            IapyxCommand::Status => {
                if let Some(controller) = model.controller.as_ref() {
                    let account_state = controller.get_account_state()?;
                    println!("-------------------------");
                    println!("- Delegation: {:?}", account_state.delegation());
                    println!("- Value: {}", account_state.value());
                    println!("- Spending counter: {}", account_state.counter());
                    println!("- Rewards: {:?}", account_state.last_rewards());
                    println!("--------------------------");
                    return Ok(());
                }
                Err(IapyxCommandError::GeneralError(
                    "wallet not recovered or generated".to_string(),
                ))
            }
            IapyxCommand::Refresh => {
                if let Some(controller) = model.controller.as_mut() {
                    controller.refresh_state()?;
                    return Ok(());
                }
                Err(IapyxCommandError::GeneralError(
                    "wallet not recovered or generated".to_string(),
                ))
            }
            IapyxCommand::Address(address) => address.exec(model),
            IapyxCommand::Logs => {
                if let Some(controller) = model.controller.as_mut() {
                    println!("{:#?}", controller.fragment_logs());
                    return Ok(());
                }
                Err(IapyxCommandError::GeneralError(
                    "wallet not recovered or generated".to_string(),
                ))
            }
        }
    }
}

#[derive(StructOpt, Debug)]
pub struct Address {
    /// blocks execution until fragment is in block
    #[structopt(short = "t", long = "testing")]
    pub testing: bool,
}

impl Address {
    pub fn exec(&self, model: &mut UserInteractionContoller) -> Result<(), IapyxCommandError> {
        if let Some(controller) = model.controller.as_mut() {
            let (prefix, discrimination) = {
                if self.testing {
                    ("ca", Discrimination::Test)
                } else {
                    ("ta", Discrimination::Production)
                }
            };
            let address =
                AddressReadable::from_address(prefix, &controller.account(discrimination));
            println!("Address: {}", address.to_string());
            return Ok(());
        }
        Err(IapyxCommandError::GeneralError(
            "wallet not recovered or generated".to_string(),
        ))
    }
}

#[derive(StructOpt, Debug)]
pub struct Vote {
    /// choice
    #[structopt(short = "c", long = "choice")]
    pub choice: String,
    /// chain proposal id
    #[structopt(short = "p", long = "id")]
    pub proposal_id: String,
}

impl Vote {
    pub fn exec(&self, model: &mut UserInteractionContoller) -> Result<(), IapyxCommandError> {
        if let Some(controller) = model.controller.as_mut() {
            let proposals = controller.get_proposals()?;
            let proposal = proposals
                .iter()
                .find(|x| x.chain_proposal_id_as_str() == self.proposal_id)
                .ok_or_else(|| {
                    IapyxCommandError::GeneralError("Cannot find proposal".to_string())
                })?;
            let choice = proposal
                .chain_vote_options
                .0
                .get(&self.choice)
                .ok_or_else(|| IapyxCommandError::GeneralError("wrong choice".to_string()))?;
            controller.vote(proposal, Choice::new(*choice))?;
            return Ok(());
        }
        Err(IapyxCommandError::GeneralError(
            "wallet not recovered or generated".to_string(),
        ))
    }
}

#[derive(StructOpt, Debug)]
pub struct Connect {
    #[structopt(short = "a", long = "address")]
    pub address: String,

    /// uses https for sending fragments
    #[structopt(short = "s", long = "use-https")]
    pub use_https_for_post: bool,

    /// uses https for sending fragments
    #[structopt(short = "d", long = "enable-debug")]
    pub enable_debug: bool,
}

impl Connect {
    pub fn exec(&self, model: &mut UserInteractionContoller) -> Result<(), IapyxCommandError> {
        let settings = RestSettings {
            use_https_for_post: self.use_https_for_post,
            enable_debug: self.enable_debug,
            ..Default::default()
        };

        if let Some(controller) = model.controller.as_mut() {
            controller.switch_backend(self.address.clone(), settings);
            return Ok(());
        }

        model.backend_address = self.address.clone();
        model.settings = settings;
        Ok(())
    }
}

#[derive(StructOpt, Debug)]
pub enum Recover {
    /// recover wallet funds from mnemonic
    Mnemonics(RecoverFromMnemonics),
    /// recover wallet funds from qr code
    Qr(RecoverFromQr),
    /// recover wallet funds from private key
    Secret(RecoverFromSecretKey),
}

impl Recover {
    pub fn exec(&self, model: &mut UserInteractionContoller) -> Result<(), IapyxCommandError> {
        match self {
            Recover::Mnemonics(mnemonics) => mnemonics.exec(model),
            Recover::Qr(qr) => qr.exec(model),
            Recover::Secret(sk) => sk.exec(model),
        }
    }
}

#[derive(StructOpt, Debug)]
pub struct RecoverFromSecretKey {
    #[structopt(short = "s", long = "secret")]
    pub input: PathBuf,
}

impl RecoverFromSecretKey {
    pub fn exec(&self, model: &mut UserInteractionContoller) -> Result<(), IapyxCommandError> {
        model.controller = Some(Controller::recover_from_sk(
            model.backend_address.clone(),
            &self.input,
            model.settings.clone(),
        )?);
        model.state = WalletState::Recovered;
        Ok(())
    }
}

#[derive(StructOpt, Debug)]
pub struct RecoverFromQr {
    #[structopt(short = "q", long = "qr")]
    pub qr_code: PathBuf,

    #[structopt(short = "p", long = "password")]
    pub password: String,
}

impl RecoverFromQr {
    pub fn exec(&self, model: &mut UserInteractionContoller) -> Result<(), IapyxCommandError> {
        model.controller = Some(Controller::recover_from_qr(
            model.backend_address.clone(),
            &self.qr_code,
            &self.password,
            model.settings.clone(),
        )?);
        model.state = WalletState::Recovered;
        Ok(())
    }
}

#[derive(StructOpt, Debug)]
pub struct RecoverFromMnemonics {
    #[structopt(short = "m", long = "mnemonics")]
    pub mnemonics: Vec<String>,
}

impl RecoverFromMnemonics {
    pub fn exec(&self, model: &mut UserInteractionContoller) -> Result<(), IapyxCommandError> {
        model.controller = Some(Controller::recover(
            model.backend_address.clone(),
            &self.mnemonics.join(" "),
            &[],
            model.settings.clone(),
        )?);
        model.state = WalletState::Recovered;
        Ok(())
    }
}

#[derive(StructOpt, Debug)]
pub struct Generate {
    /// Words count
    #[structopt(short = "w", long = "words")]
    pub count: usize,
}

impl Generate {
    pub fn exec(&self, model: &mut UserInteractionContoller) -> Result<(), IapyxCommandError> {
        model.controller = Some(Controller::generate(
            model.backend_address.clone(),
            Type::from_word_count(self.count)?,
            model.settings.clone(),
        )?);
        model.state = WalletState::Generated;
        Ok(())
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Error, Debug)]
pub enum IapyxCommandError {
    #[error("{0}")]
    GeneralError(String),
    #[error("{0}")]
    ControllerError(#[from] crate::controller::ControllerError),
    #[error("wrong word count for generating wallet")]
    GenerateWalletError(#[from] bip39::Error),
}
