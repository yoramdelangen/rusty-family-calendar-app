use std::{
    error::Error,
    io::{self, Write},
};

use argh::FromArgs;

use crate::calendar::{self, Config, ConfigProfile};

#[derive(FromArgs)]
#[argh(subcommand, name = "profile")]
/// Manage profiles
pub(crate) struct ProfileArgs {
    #[argh(subcommand)]
    pub(crate) command: Option<ProfileCommand>,
}

#[derive(FromArgs)]
#[argh(subcommand)]
pub(crate) enum ProfileCommand {
    Add(ProfileAddArgs),
}

#[derive(FromArgs)]
#[argh(subcommand, name = "add")]
/// Add a profile
pub(crate) struct ProfileAddArgs {}

pub(crate) fn list_profiles() -> Result<(), Box<dyn Error>> {
    let config = calendar::read_config()?;

    if config.profile.is_empty() {
        println!("no profiles configured");
        return Ok(());
    }

    let rows = config
        .profile
        .iter()
        .enumerate()
        .map(|(index, profile)| vec![
            (index + 1).to_string(),
            profile.name.clone(),
            profile.calendar.len().to_string(),
        ])
        .collect::<Vec<_>>();

    crate::table::print(&["#", "Name", "Calendars"], &rows);

    Ok(())
}

pub(crate) fn profile_add() -> Result<(), Box<dyn Error>> {
    let mut config = calendar::read_config()?;
    let name = prompt_unique_profile_name(&config)?;

    config.profile.push(ConfigProfile {
        name: name.clone(),
        calendar: Vec::new(),
    });
    calendar::save_config(&config)?;

    println!("added profile={name}");
    Ok(())
}

fn prompt_unique_profile_name(config: &Config) -> Result<String, Box<dyn Error>> {
    loop {
        let name = prompt_required("Profile name")?;
        if config.profile.iter().any(|profile| profile.name == name) {
            println!("profile already exists");
            continue;
        }

        return Ok(name);
    }
}

fn prompt_required(label: &str) -> Result<String, Box<dyn Error>> {
    loop {
        let value = prompt(label)?;
        if !value.is_empty() {
            return Ok(value);
        }

        println!("value is required");
    }
}

fn prompt(label: &str) -> Result<String, Box<dyn Error>> {
    print!("{}: ", label);
    io::stdout().flush()?;

    let mut input = String::new();
    if io::stdin().read_line(&mut input)? == 0 {
        return Err(other_error("unexpected end of input"));
    }

    Ok(input.trim().to_owned())
}

fn other_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::other(message.into()))
}
