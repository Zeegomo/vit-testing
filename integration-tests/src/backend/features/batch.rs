use crate::common::{iapyx_from_secret_key, vitup_setup, VoteTiming};
use assert_fs::TempDir;
use chain_impl_mockchain::vote::Choice;
use iapyx::utils::valid_until::ValidUntil;
use jormungandr_lib::interfaces::FragmentStatus;
use jormungandr_testing_utils::testing::node::time;
use valgrind::Protocol;
use vit_servicing_station_tests::common::data::ArbitraryValidVotingTemplateGenerator;
use vitup::config::{InitialEntry, Initials};
use vitup::scenario::network::setup_network;
use vitup::setup::start::quick::QuickVitBackendSettingsBuilder;
const PIN: &str = "1234";
const ALICE: &str = "alice";

#[test]
pub fn transactions_are_send_between_nodes_with_correct_order() {
    let testing_directory = TempDir::new().unwrap().into_persistent();
    let endpoint = "127.0.0.1:8080";
    let version = "2.0";
    let batch_size = 10;
    let vote_timing = VoteTiming::new(0, 1, 2);

    let mut quick_setup = QuickVitBackendSettingsBuilder::new();
    quick_setup
        .initials(Initials(vec![InitialEntry::Wallet {
            name: ALICE.to_string(),
            funds: 10_000,
            pin: PIN.to_string(),
        }]))
        .vote_start_epoch(vote_timing.vote_start)
        .tally_start_epoch(vote_timing.tally_start)
        .tally_end_epoch(vote_timing.tally_end)
        .slot_duration_in_seconds(2)
        .slots_in_epoch_count(60)
        .proposals_count(300)
        .voting_power(31_000)
        .private(true);

    let mut template_generator = ArbitraryValidVotingTemplateGenerator::new();
    let (mut vit_controller, mut controller, vit_parameters, _fund_name) =
        vitup_setup(quick_setup, testing_directory.path().to_path_buf());

    let (nodes, vit_station, wallet_proxy) = setup_network(
        &mut controller,
        &mut vit_controller,
        vit_parameters,
        &mut template_generator,
        endpoint.to_string(),
        &Protocol::Http,
        version.to_owned(),
    )
    .unwrap();

    let secret = testing_directory.path().join("vit_backend/wallet_alice");
    let mut alice = iapyx_from_secret_key(secret, &wallet_proxy).unwrap();

    let proposals = alice.proposals().unwrap();
    let votes_data = proposals
        .iter()
        .take(batch_size)
        .map(|proposal| (proposal, Choice::new(0)))
        .collect();

    let valid_until = ValidUntil::BySlotShift(5);
    let expiry_date = valid_until
        .clone()
        .into_expiry_date(Some(alice.settings().unwrap()))
        .unwrap();
    let fragment_ids = alice
        .votes_batch(votes_data, &ValidUntil::BySlotShift(5))
        .unwrap()
        .iter()
        .map(|item| item.to_string())
        .collect();

    time::wait_for_date(expiry_date.into(), nodes[0].rest());

    let statuses = nodes[0].rest().fragments_statuses(fragment_ids).unwrap();

    vit_station.shutdown();
    wallet_proxy.shutdown();
    for node in nodes {
        node.shutdown().unwrap();
    }
    controller.finalize();

    assert!(statuses
        .iter()
        .all(|(_, status)| matches!(status, FragmentStatus::InABlock { .. })));
}