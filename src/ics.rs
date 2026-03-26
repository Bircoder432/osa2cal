use anyhow::Result;
use chrono::Timelike;
use icalendar::{Calendar, Component, Event, EventLike};
use osars::Schedule;

pub async fn generate_ics(schedules: &[Schedule], config: &super::Config) -> Result<String> {
    let mut calendar = Calendar::new();

    for schedule in schedules {
        for lesson in &schedule.lessons {
            let start = schedule
                .date
                .and_hms_opt(lesson.start_time.hour(), lesson.start_time.minute(), 0)
                .unwrap();

            let end = schedule
                .date
                .and_hms_opt(lesson.end_time.hour(), lesson.end_time.minute(), 0)
                .unwrap();

            let location = format!(
                "{} - {}",
                config.college_name.as_deref().unwrap_or("College"),
                lesson.cabinet
            );

            let uid = format!(
                "osa2cal-{}-{}-{}@osa2cal",
                schedule.group_id, schedule.date, lesson.order
            );

            let event = Event::new()
                .summary(&lesson.title)
                .description(&format!(
                    "Teacher: {}\nCabinet: {}",
                    lesson.teacher, lesson.cabinet
                ))
                .location(&location)
                .starts(start)
                .ends(end)
                .uid(&uid)
                .done();

            calendar.push(event);
        }
    }

    Ok(calendar.to_string())
}
