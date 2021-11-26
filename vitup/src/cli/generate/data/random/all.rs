use crate::builders::post_deployment::generate_random_database;
use crate::builders::post_deployment::DeploymentTree;
use crate::builders::VitBackendSettingsBuilder;
use crate::builders::{utils::io::read_config, utils::ContextExtension};
use crate::Result;
use jormungandr_scenario_tests::Context;
use std::path::PathBuf;
use structopt::StructOpt;
#[derive(StructOpt, Debug)]
#[structopt(setting = structopt::clap::AppSettings::ColoredHelp)]
pub struct AllRandomDataCommandArgs {
    /// Careful! directory would be removed before export
    #[structopt(long = "output", default_value = "./data")]
    pub output_directory: PathBuf,

    /// how many qr to generate
    #[structopt(long = "config")]
    pub config: PathBuf,

    #[structopt(long = "snapshot")]
    pub snapshot: Option<PathBuf>,
}

impl AllRandomDataCommandArgs {
    pub fn exec(self) -> Result<()> {
        std::env::set_var("RUST_BACKTRACE", "full");

        let context = Context::empty_from_dir(&self.output_directory);

        let mut quick_setup = VitBackendSettingsBuilder::new();
        let mut config = read_config(&self.config)?;

        if let Some(snapshot) = self.snapshot {
            config.extend_from_initials_file(snapshot)?;
        }

        quick_setup.upload_parameters(config.params.clone());
        quick_setup.fees(config.linear_fees);
        quick_setup.set_external_committees(config.committees);
        quick_setup.consensus_leaders_ids(config.consensus_leader_ids);

        if !self.output_directory.exists() {
            std::fs::create_dir_all(&self.output_directory)?;
        }

        let deployment_tree = DeploymentTree::new(&self.output_directory, quick_setup.title());

        let (_, controller, vit_parameters, _) = quick_setup.build(context)?;

        generate_random_database(&deployment_tree, vit_parameters);

        println!(
            "voteplan ids: {:?}",
            controller
                .vote_plans()
                .iter()
                .map(|x| x.id())
                .collect::<Vec<String>>()
        );

        quick_setup.print_report();
        Ok(())
    }
}