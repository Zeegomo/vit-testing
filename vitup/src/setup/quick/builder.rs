use super::QuickVitBackendParameters;
use crate::scenario::controller::VitController;
use crate::scenario::controller::VitControllerBuilder;
use crate::Result;
use assert_fs::fixture::PathChild;
use chain_impl_mockchain::testing::scenario::template::VotePlanDef;
use chain_impl_mockchain::vote::PayloadType;
use chain_impl_mockchain::{
    testing::scenario::template::{ProposalDefBuilder, VotePlanDefBuilder},
    value::Value,
};
use chain_vote::committee::ElectionPublicKey;
use chrono::naive::NaiveDateTime;
use jormungandr_lib::time::SecondsSinceUnixEpoch;
use jormungandr_scenario_tests::scenario::settings::Settings;
use jormungandr_scenario_tests::scenario::{
    ActiveSlotCoefficient, ConsensusVersion, ContextChaCha, Controller, KESUpdateSpeed, Milli,
    NumberOfSlotsPerEpoch, SlotDuration, TopologyBuilder,
};
use jormungandr_testing_utils::testing::network_builder::{
    Blockchain, Node, WalletTemplate, WalletType,
};
use jormungandr_testing_utils::wallet::ElectionPublicKeyExtension;
use vit_servicing_station_tests::common::data::ValidVotePlanParameters;

pub const LEADER_1: &str = "Leader1";
pub const LEADER_2: &str = "Leader2";
pub const LEADER_3: &str = "Leader3";
pub const LEADER_4: &str = "Leader4";
pub const WALLET_NODE: &str = "Wallet_Node";

#[derive(Clone)]
pub struct QuickVitBackendSettingsBuilder {
    parameters: QuickVitBackendParameters,
    committe_wallet_name: String,
    title: String,
}

impl Default for QuickVitBackendSettingsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

const FORMAT: &str = "%Y-%m-%d %H:%M:%S";

impl QuickVitBackendSettingsBuilder {
    pub fn new() -> Self {
        Self {
            parameters: Default::default(),
            title: "vit_backend".to_owned(),
            committe_wallet_name: "committee".to_owned(),
        }
    }

    pub fn parameters(&self) -> &QuickVitBackendParameters {
        &self.parameters
    }

    pub fn title(&self) -> String {
        self.title.clone()
    }

    pub fn initials(&mut self, initials: Vec<u64>) -> &mut Self {
        self.parameters.initials = initials;
        self
    }

    pub fn initials_count(&mut self, initials_count: usize) -> &mut Self {
        let initials: Vec<u64> = std::iter::from_fn(|| Some(10_000))
            .take(initials_count)
            .collect();
        self.initials(initials);
        self
    }

    pub fn vote_start_epoch(&mut self, vote_start_epoch: u32) -> &mut Self {
        self.parameters.vote_start = vote_start_epoch as u64;
        self
    }

    pub fn tally_start_epoch(&mut self, tally_start_epoch: u32) -> &mut Self {
        self.parameters.vote_tally = tally_start_epoch as u64;
        self
    }
    pub fn tally_end_epoch(&mut self, tally_end_epoch: u32) -> &mut Self {
        self.parameters.tally_end = tally_end_epoch as u64;
        self
    }

    pub fn slot_duration_in_seconds(&mut self, slot_duration: u8) -> &mut Self {
        self.parameters.slot_duration = slot_duration;
        self
    }
    pub fn slots_in_epoch_count(&mut self, slots_in_epoch: u32) -> &mut Self {
        self.parameters.slots_per_epoch = slots_in_epoch;
        self
    }
    pub fn proposals_count(&mut self, proposals_count: u32) -> &mut Self {
        self.parameters.proposals = proposals_count;
        self
    }
    pub fn voting_power(&mut self, voting_power: u64) -> &mut Self {
        self.parameters.voting_power = voting_power * 1_000_000;
        self
    }

    pub fn next_vote_timestamp(&mut self, next_vote_timestamp: Option<String>) -> &mut Self {
        if let Some(timestamp) = next_vote_timestamp {
            self.parameters.next_vote_start_time =
                Some(NaiveDateTime::parse_from_str(&timestamp, FORMAT).unwrap());
        }
        self
    }

    pub fn vote_start_timestamp(&mut self, vote_start_timestamp: Option<String>) -> &mut Self {
        if let Some(timestamp) = vote_start_timestamp {
            self.parameters.vote_start_timestamp =
                Some(NaiveDateTime::parse_from_str(&timestamp, FORMAT).unwrap());
        }
        self
    }

    pub fn tally_start_timestamp(&mut self, tally_start_timestamp: Option<String>) -> &mut Self {
        if let Some(timestamp) = tally_start_timestamp {
            self.parameters.tally_start_timestamp =
                Some(NaiveDateTime::parse_from_str(&timestamp, FORMAT).unwrap());
        }
        self
    }

    pub fn tally_end_timestamp(&mut self, tally_end_timestamp: Option<String>) -> &mut Self {
        if let Some(timestamp) = tally_end_timestamp {
            self.parameters.tally_end_timestamp =
                Some(NaiveDateTime::parse_from_str(&timestamp, FORMAT).unwrap());
        }
        self
    }

    pub fn fund_name(&self) -> String {
        self.parameters.fund_name.to_string()
    }

    pub fn private(&mut self, private: bool) {
        self.parameters.private = private;
    }

    pub fn recalculate_voting_periods_if_needed(&mut self, block0_date: SecondsSinceUnixEpoch) {
        let epoch_duration: u64 =
            self.parameters.slot_duration as u64 * self.parameters.slots_per_epoch as u64;
        if self.parameters.vote_start_timestamp.is_none() {
            println!(
                "Current date {:?}",
                NaiveDateTime::from_timestamp(block0_date.to_secs() as i64, 0)
            );
            let vote_start_timestamp =
                block0_date.to_secs() + epoch_duration * self.parameters.vote_start;
            self.parameters.vote_start_timestamp = Some(NaiveDateTime::from_timestamp(
                vote_start_timestamp as i64,
                0,
            ));
            let tally_start_timestamp =
                block0_date.to_secs() + epoch_duration * self.parameters.vote_tally;
            self.parameters.tally_start_timestamp = Some(NaiveDateTime::from_timestamp(
                tally_start_timestamp as i64,
                0,
            ));
            let tally_end_timestamp =
                block0_date.to_secs() + epoch_duration * self.parameters.tally_end;
            self.parameters.tally_end_timestamp =
                Some(NaiveDateTime::from_timestamp(tally_end_timestamp as i64, 0));
        }

        if self.parameters.next_vote_start_time.is_none() {
            let timestamp = SecondsSinceUnixEpoch::now().to_secs()
                + epoch_duration * self.parameters.tally_end
                + 10;
            self.parameters.next_vote_start_time =
                Some(NaiveDateTime::from_timestamp(timestamp as i64, 0));
        }
    }

    pub fn upload_parameters(&mut self, parameters: QuickVitBackendParameters) {
        self.parameters = parameters;
    }

    pub fn vote_plan_parameters(
        &self,
        vote_plan: VotePlanDef,
        settings: &Settings,
    ) -> ValidVotePlanParameters {
        let mut parameters = ValidVotePlanParameters::new(vote_plan);
        parameters.set_voting_power_threshold(self.parameters.voting_power as i64);
        parameters.set_voting_start(self.parameters.vote_start_timestamp.unwrap().timestamp());
        parameters
            .set_voting_tally_start(self.parameters.tally_start_timestamp.unwrap().timestamp());
        parameters.set_voting_tally_end(self.parameters.tally_end_timestamp.unwrap().timestamp());
        parameters
            .set_next_fund_start_time(self.parameters.next_vote_start_time.unwrap().timestamp());

        if self.parameters.private {
            let mut committee_wallet = settings
                .network_settings
                .wallets
                .get(&self.committe_wallet_name)
                .unwrap()
                .clone();
            let identifier = committee_wallet.identifier();
            let private_key_data = settings
                .private_vote_plans
                .values()
                .next()
                .unwrap()
                .get(&identifier.into())
                .unwrap();
            let key: ElectionPublicKey = private_key_data.encrypting_vote_key();
            parameters.set_vote_encryption_key(key.to_base32().unwrap());
        }
        parameters
    }

    pub fn build(
        &mut self,
        mut context: ContextChaCha,
    ) -> Result<(VitController, Controller, ValidVotePlanParameters)> {
        let mut builder = VitControllerBuilder::new(&self.title);
        let mut topology_builder = TopologyBuilder::new();

        // Leader 1
        let leader_1 = Node::new(LEADER_1);
        topology_builder.register_node(leader_1);

        // leader 2
        let mut leader_2 = Node::new(LEADER_2);
        leader_2.add_trusted_peer(LEADER_1);
        topology_builder.register_node(leader_2);

        // leader 3
        let mut leader_3 = Node::new(LEADER_3);
        leader_3.add_trusted_peer(LEADER_1);
        leader_3.add_trusted_peer(LEADER_2);
        topology_builder.register_node(leader_3);

        // leader 4
        let mut leader_4 = Node::new(LEADER_4);
        leader_4.add_trusted_peer(LEADER_1);
        leader_4.add_trusted_peer(LEADER_2);
        leader_4.add_trusted_peer(LEADER_3);
        topology_builder.register_node(leader_4);

        // passive
        let mut passive = Node::new(WALLET_NODE);
        passive.add_trusted_peer(LEADER_1);
        passive.add_trusted_peer(LEADER_2);
        passive.add_trusted_peer(LEADER_3);
        passive.add_trusted_peer(LEADER_4);

        topology_builder.register_node(passive);

        builder.set_topology(topology_builder.build());

        let mut blockchain = Blockchain::new(
            ConsensusVersion::Bft,
            NumberOfSlotsPerEpoch::new(self.parameters.slots_per_epoch)
                .expect("valid number of slots per epoch"),
            SlotDuration::new(self.parameters.slot_duration)
                .expect("valid slot duration in seconds"),
            KESUpdateSpeed::new(46800).expect("valid kes update speed in seconds"),
            ActiveSlotCoefficient::new(Milli::from_millis(700))
                .expect("active slot coefficient in millis"),
        );

        blockchain.add_leader(LEADER_1);
        blockchain.add_leader(LEADER_2);
        blockchain.add_leader(LEADER_3);
        blockchain.add_leader(LEADER_4);

        let committe_wallet =
            WalletTemplate::new_account(&self.committe_wallet_name, Value(1_000_000));
        blockchain.add_wallet(committe_wallet);
        let mut i = 1u32;

        let child = context.child_directory(self.title());

        for initial in self.parameters.initials.iter() {
            let wallet_alias = format!("wallet_{}_with_{}", i, initial);
            let wallet = WalletTemplate::new_utxo(wallet_alias.clone(), Value(*initial));
            blockchain.add_wallet(wallet);
            i += 1;
        }

        blockchain.add_committee(&self.committe_wallet_name);

        let mut vote_plan_builder = VotePlanDefBuilder::new(&self.fund_name());
        vote_plan_builder.owner(&self.committe_wallet_name);

        if self.parameters.private {
            vote_plan_builder.payload_type(PayloadType::Private);
        }
        vote_plan_builder.vote_phases(
            self.parameters.vote_start as u32,
            self.parameters.vote_tally as u32,
            self.parameters.tally_end as u32,
        );

        for _ in 0..self.parameters.proposals {
            let mut proposal_builder = ProposalDefBuilder::new(
                chain_impl_mockchain::testing::VoteTestGen::external_proposal_id(),
            );
            proposal_builder.options(3);

            proposal_builder.action_off_chain();
            vote_plan_builder.with_proposal(&mut proposal_builder);
        }

        let vote_plan = vote_plan_builder.build();
        blockchain.add_vote_plan(vote_plan.clone());
        builder.set_blockchain(blockchain);
        builder.build_settings(&mut context);

        let (vit_controller, controller) = builder.build_controllers(context)?;

        let password = "1234".to_owned();

        for (alias, _template) in controller
            .wallets()
            .filter(|(_, x)| *x.template().wallet_type() == WalletType::UTxO)
        {
            let wallet = controller.wallet(alias).unwrap();
            let png = child.child(format!("{}_{}.png", alias, password));
            wallet.save_qr_code(png.path(), password.as_bytes());
        }

        controller.settings().dump_private_vote_keys(child);

        self.recalculate_voting_periods_if_needed(
            controller
                .settings()
                .network_settings
                .block0
                .blockchain_configuration
                .block0_date,
        );

        let parameters = self.vote_plan_parameters(vote_plan, &controller.settings());

        Ok((vit_controller, controller, parameters))
    }
}
