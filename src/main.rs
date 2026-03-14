use anyhow::Result;
use chrono::{Datelike, NaiveDateTime};
use clap::{Parser, Subcommand};
use colored::Colorize;
use hex;
use indicatif::ProgressBar;
use osars::Client;
use sha1::{Digest, Sha1};
use std::fs;
use std::path::PathBuf;

mod caldav;
mod config;
mod ics;

use caldav::CalDavClient;
use config::Config;

#[derive(Parser)]
#[command(name = "osa2cal")]
#[command(about = "OpenScheduleAPI to Calendar converter")]
#[command(version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// API base URL
    #[arg(short, long, global = true)]
    api_url: Option<String>,

    /// Group ID
    #[arg(short, long, global = true)]
    group: Option<u32>,

    /// Verbose output
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Configure settings (CalDAV credentials, defaults)
    Config {
        /// CalDAV server URL
        #[arg(short, long)]
        caldav_url: Option<String>,
        /// CalDAV username
        #[arg(short, long)]
        username: Option<String>,
        /// CalDAV password
        #[arg(short, long)]
        password: Option<String>,
        /// Default group ID
        #[arg(short, long)]
        default_group: Option<u32>,
        /// API base URL
        #[arg(short, long)]
        api_url: Option<String>,
    },

    /// Export schedule to ICS file
    Export {
        /// Output file path
        #[arg(short, long, default_value = "schedule.ics")]
        output: PathBuf,
        /// Export period: today, tomorrow, week, month, all
        #[arg(short, long, default_value = "month")]
        period: String,
    },

    /// Sync schedule to CalDAV server
    Sync {
        /// Sync period: today, tomorrow, week, month, all
        #[arg(short, long, default_value = "month")]
        period: String,
        /// Dry run (show what would be done)
        #[arg(long)]
        dry_run: bool,
        /// Force: try to auto-create calendar if not exists
        #[arg(long)]
        force: bool,
        /// Calendar ID/name (overrides config)
        #[arg(short, long)]
        calendar_id: Option<String>,
    },

    /// Show schedule in terminal
    Show {
        /// Show period: today, tomorrow, week
        #[arg(short, long, default_value = "week")]
        period: String,
    },

    /// List available colleges, campuses and groups
    List {
        #[command(subcommand)]
        target: ListTarget,
    },
}

#[derive(Subcommand)]
enum ListTarget {
    /// List all colleges
    Colleges,
    /// List campuses for a college
    Campuses { college_id: u32 },
    /// List groups for a campus
    Groups { campus_id: u32 },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let mut config = Config::load().unwrap_or_default();

    if let Some(url) = cli.api_url {
        config.api_url = Some(url);
    }
    if let Some(group) = cli.group {
        config.default_group = Some(group);
    }

    match cli.command {
        Commands::Config {
            caldav_url,
            username,
            password,
            default_group,
            api_url,
        } => {
            if let Some(url) = caldav_url {
                config.caldav_url = Some(url);
                println!("{} CalDAV URL set", "✓".green());
            }
            if let Some(user) = username {
                config.caldav_username = Some(user);
                println!("{} Username set", "✓".green());
            }
            if let Some(pass) = password {
                config.caldav_password = Some(pass);
                println!("{} Password set", "✓".green());
            }
            if let Some(group) = default_group {
                config.default_group = Some(group);
                println!("{} Default group set to {}", "✓".green(), group);
            }
            if let Some(url) = api_url {
                config.api_url = Some(url);
                println!("{} API URL set", "✓".green());
            }
            config.save()?;
            println!("{} Configuration saved", "✓".green());
        }

        Commands::Export { output, period } => {
            let api_url = config.api_url.as_ref().ok_or_else(|| {
                anyhow::anyhow!("API URL not configured. Run: osa2cal config --api-url <URL>")
            })?;
            let group_id = config.default_group.ok_or_else(|| {
                anyhow::anyhow!("Group ID not specified. Use --group or configure default")
            })?;

            println!("{} Fetching schedule...", "⏳".yellow());
            let client = Client::new(api_url);
            let schedules = fetch_schedules(&client, group_id, &period).await?;

            println!("{} Generating ICS file...", "⏳".yellow());
            let ics_content = ics::generate_ics(&schedules, &config).await?;
            fs::write(&output, ics_content)?;
            println!("{} Saved to {}", "✓".green(), output.display());
        }

        Commands::Sync {
            period,
            dry_run,
            force,
            calendar_id,
        } => {
            let api_url = config
                .api_url
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("API URL not configured"))?;
            let group_id = config
                .default_group
                .ok_or_else(|| anyhow::anyhow!("Group ID not specified"))?;
            let caldav_url = config
                .caldav_url
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("CalDAV URL not configured"))?;
            let username = config
                .caldav_username
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("CalDAV username not configured"))?;
            let password = config
                .caldav_password
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("CalDAV password not configured"))?;

            let cal_id = calendar_id
                .or_else(|| config.calendar_name.clone())
                .unwrap_or_else(|| "default".to_string());

            println!("{} Fetching schedule from API...", "⏳".yellow());
            let client = Client::new(api_url);
            let schedules = fetch_schedules(&client, group_id, &period).await?;

            if schedules.is_empty() {
                println!(
                    "{} No schedule found for the specified period",
                    "⚠".yellow()
                );
                return Ok(());
            }

            println!("{} Connecting to CalDAV server...", "⏳".yellow());
            let caldav = CalDavClient::new(caldav_url, username, password).await?;

            if dry_run {
                println!(
                    "{} Dry run mode - would sync {} events to calendar '{}'",
                    "ℹ".blue(),
                    schedules.len(),
                    cal_id
                );
                return Ok(());
            }

            let cal_exists = caldav.calendar_exists(&cal_id).await.unwrap_or(false);

            if !cal_exists {
                if force {
                    println!(
                        "{} Calendar '{}' not found, attempting to create...",
                        "⏳".yellow(),
                        cal_id
                    );
                    match caldav.create_calendar(&cal_id, &cal_id).await {
                        Ok(_) => println!("{} Calendar created", "✓".green()),
                        Err(e) => {
                            println!("{} Cannot create calendar: {}", "✗".red(), e);
                            println!(
                                "{} Please create calendar '{}' manually in web interface",
                                "ℹ".blue(),
                                cal_id
                            );
                            std::process::exit(1);
                        }
                    }
                } else {
                    println!("{} Calendar '{}' not found", "✗".red(), cal_id);
                    println!(
                        "{} Use --force to try auto-create, or create it manually",
                        "ℹ".blue()
                    );
                    anyhow::bail!("Calendar does not exist: {}", cal_id);
                }
            } else {
                println!("{} Using calendar '{}'", "✓".green(), cal_id);
            }

            let pb = ProgressBar::new(schedules.len() as u64);

            let mut synced = 0;
            for schedule in &schedules {
                for lesson in &schedule.lessons {
                    let event = create_caldav_event(schedule, lesson, &config).await?;
                    caldav.put_event(&cal_id, &event).await?;
                    synced += 1;
                }
                pb.inc(1);
            }
            pb.finish_with_message("Done");

            println!(
                "{} Synced {} events to calendar '{}'",
                "✓".green(),
                synced,
                cal_id
            );
        }

        Commands::Show { period } => {
            let api_url = config
                .api_url
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("API URL not configured"))?;
            let group_id = config
                .default_group
                .ok_or_else(|| anyhow::anyhow!("Group ID not specified"))?;

            let client = Client::new(api_url);
            let schedules = fetch_schedules(&client, group_id, &period).await?;

            print_schedule(&schedules);
        }

        Commands::List { target } => {
            let api_url = config
                .api_url
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("API URL not configured"))?;
            let client = Client::new(api_url);

            match target {
                ListTarget::Colleges => {
                    let colleges = client.colleges().send().await?;
                    println!("{:<10} {}", "ID", "Name");
                    println!("{}", "-".repeat(50));
                    for college in colleges {
                        println!("{:<10} {}", college.college_id, college.name);
                    }
                }
                ListTarget::Campuses { college_id } => {
                    let campuses = client
                        .colleges()
                        .college(college_id)
                        .campuses()
                        .send()
                        .await?;
                    println!("{:<10} {}", "ID", "Name");
                    println!("{}", "-".repeat(50));
                    for campus in campuses {
                        println!("{:<10} {}", campus.id, campus.name);
                    }
                }
                ListTarget::Groups { campus_id } => {
                    let groups = client.groups(campus_id).send().await?;
                    println!("{:<10} {}", "ID", "Name");
                    println!("{}", "-".repeat(50));
                    for group in groups {
                        println!("{:<10} {}", group.id, group.name);
                    }
                }
            }
        }
    }

    Ok(())
}

async fn fetch_schedules(
    client: &Client,
    group_id: u32,
    period: &str,
) -> Result<Vec<osars::Schedule>> {
    let query = client.schedule(group_id);

    let schedules = match period {
        "today" => query.today().send().await?,
        "tomorrow" => query.tomorrow().send().await?,
        "week" => query.week(osars::Week::Current).send().await?,
        "month" => {
            let mut all = Vec::new();
            all.extend(query.week(osars::Week::Current).send().await?);
            all.extend(
                client
                    .schedule(group_id)
                    .week(osars::Week::Next)
                    .send()
                    .await?,
            );
            all
        }
        "all" => {
            let mut all = Vec::new();
            all.extend(
                client
                    .schedule(group_id)
                    .week(osars::Week::Previous)
                    .send()
                    .await?,
            );
            all.extend(query.week(osars::Week::Current).send().await?);
            all.extend(
                client
                    .schedule(group_id)
                    .week(osars::Week::Next)
                    .send()
                    .await?,
            );
            all
        }
        date if date.contains('-') => query.date(date).send().await?,
        _ => anyhow::bail!(
            "Unknown period: {}. Use: today, tomorrow, week, month, all, or YYYY-MM-DD",
            period
        ),
    };

    Ok(schedules)
}

fn print_schedule(schedules: &[osars::Schedule]) {
    for schedule in schedules {
        let date = schedule.date;
        let weekday = date.weekday();
        println!(
            "\n{} {} ({})",
            date.to_string().bold().underline(),
            format!("{:?}", weekday).cyan(),
            schedule.lessons.len()
        );

        for lesson in &schedule.lessons {
            let time = format!(
                "{}-{}",
                lesson.start_time.format("%H:%M"),
                lesson.end_time.format("%H:%M")
            )
            .yellow();

            println!(
                "  {} | {} | {} | {}",
                time,
                lesson.title.bold(),
                lesson.cabinet.green(),
                lesson.teacher.dimmed()
            );
        }
    }
}

async fn create_caldav_event(
    schedule: &osars::Schedule,
    lesson: &osars::Lesson,
    config: &Config,
) -> Result<caldav::Event> {
    let start = NaiveDateTime::new(schedule.date, lesson.start_time);
    let end = NaiveDateTime::new(schedule.date, lesson.end_time);

    // Location: College - Cabinet
    let location = format!(
        "{} - {}",
        config.college_name.as_deref().unwrap_or("College"),
        lesson.cabinet
    );

    let description = format!(
        "Teacher: {}\nGroup ID: {}",
        lesson.teacher, schedule.group_id
    );

    let title_hash = {
        let mut hasher = Sha1::new();
        hasher.update(lesson.title.as_bytes());
        hex::encode(hasher.finalize())
    };

    let uid = format!(
        "osa2cal-{}-{}-{}-{}",
        schedule.group_id, schedule.date, lesson.order, title_hash
    );

    Ok(caldav::Event {
        uid,
        summary: lesson.title.clone(),
        location: Some(location),
        description: Some(description),
        start,
        end,
        timezone: config
            .timezone
            .clone()
            .unwrap_or("Europe/Moscow".to_string()),
    })
}
