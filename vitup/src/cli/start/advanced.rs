use crate::builders::utils::io::{read_config, read_initials};
use crate::builders::VitBackendSettingsBuilder;
use crate::config::mode::{parse_mode_from_str, Mode};
use crate::scenario::spawn::spawn_network;
use crate::scenario::spawn::NetworkSpawnParams;
use crate::{error::Error, Result};
use hersir::config::SessionSettings;
use jormungandr_automation::jormungandr::LogLevel;
use std::path::PathBuf;
use std::str::FromStr;
use structopt::StructOpt;
use vit_servicing_station_tests::common::data::ExternalValidVotingTemplateGenerator;

#[derive(StructOpt, Debug)]
#[structopt(setting = structopt::clap::AppSettings::ColoredHelp)]
pub struct AdvancedStartCommandArgs {
    /// path or name of the jormungandr node to test
    #[structopt(long = "jormungandr", default_value = "jormungandr")]
    pub jormungandr: PathBuf,

    /// path or name of the jcli to test
    #[structopt(long = "jcli", default_value = "jcli")]
    pub jcli: PathBuf,

    /// set a directory in which the tests will be run, allowing every details
    /// to be save persistently. By default it will create temporary directories
    /// and will delete the files and documents
    #[structopt(long = "root-dir", default_value = ".")]
    pub testing_directory: PathBuf,
    /// level for all nodes
    #[structopt(long = "log-level", default_value = "info")]
    pub log_level: String,

    /// interactive mode introduce easy way to interact with backend
    /// is capable of quering logs, sending transactions (e.g. tallying), etc.,
    #[structopt(
        long = "mode",
        default_value = "Endless",
        parse(from_str = parse_mode_from_str)
    )]
    pub mode: Mode,

    /// endopint in format: 127.0.0.1:80
    #[structopt(long = "endpoint", default_value = "0.0.0.0:80")]
    pub endpoint: String,

    /// token, only applicable if service mode is used
    #[structopt(long = "token")]
    pub token: Option<String>,

    /// how many qr to generate
    #[structopt(long = "config")]
    pub config: PathBuf,

    /// proposals import json
    #[structopt(
        long = "proposals",
        default_value = "../../catalyst-resources/ideascale/fund6/proposals.json"
    )]
    pub proposals: PathBuf,

    /// challenges import json
    #[structopt(
        long = "challenges",
        default_value = "../../catalyst-resources/ideascale/fund6/challenges.json"
    )]
    pub challenges: PathBuf,

    /// challenges import json
    #[structopt(
        long = "reviews",
        default_value = "../../catalyst-resources/ideascale/fund6/reviews.json"
    )]
    pub reviews: PathBuf,

    /// funds import json
    #[structopt(
        long = "funds",
        default_value = "../../catalyst-resources/ideascale/fund6/funds.json"
    )]
    pub funds: PathBuf,

    #[structopt(long = "snapshot")]
    pub snapshot: Option<PathBuf>,
}

impl AdvancedStartCommandArgs {
    pub fn exec(self) -> Result<()> {
        std::env::set_var("RUST_BACKTRACE", "full");

        // TODO: what happened to this function? should be re-implemented? does it have a
        // replacement?
        // as far as I can see it was only used to verify that the binary exists.
        // let jormungandr = prepare_command(&self.jormungandr);
        let jormungandr = &self.jormungandr;
        let mut testing_directory = self.testing_directory;
        let generate_documentation = true;
        let log_level = self.log_level;
        let mode = self.mode;
        let endpoint = self.endpoint;
        let token = self.token;

        let session_settings = SessionSettings {
            jormungandr: Some(jormungandr.to_path_buf()),
            root: testing_directory.clone().into(),
            generate_documentation,
            mode: mode.into(),
            log: LogLevel::from_str(&log_level)
                .map_err(|_| Error::UnknownLogLevel(log_level.clone()))?,
            title: "advanced".to_string(),
        };

        let mut config = read_config(&self.config)?;

        if let Some(snapshot) = self.snapshot {
            config
                .params
                .initials
                .extend_from_external(read_initials(snapshot)?);
        }

        let mut quick_setup = VitBackendSettingsBuilder::new();
        quick_setup.upload_parameters(config.params.clone());
        quick_setup.fees(config.linear_fees);
        quick_setup.set_external_committees(config.committees);

        let mut template_generator = ExternalValidVotingTemplateGenerator::new(
            self.proposals,
            self.challenges,
            self.funds,
            self.reviews,
        )
        .unwrap();

        testing_directory.push(quick_setup.title());
        if testing_directory.exists() {
            std::fs::remove_dir_all(&testing_directory)?;
        }

        let network_spawn_params = NetworkSpawnParams::new(
            endpoint,
            &quick_setup.parameters(),
            token,
            testing_directory,
        );
        spawn_network(
            mode,
            session_settings,
            network_spawn_params,
            &mut template_generator,
            quick_setup,
        )?;
        Ok(())
    }
}
