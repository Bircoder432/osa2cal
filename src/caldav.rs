#[allow(dead_code)]
use anyhow::Result;
use chrono::{NaiveDateTime, Utc};
use colored::Colorize;
use url::Url;

pub struct CalDavClient {
    client: reqwest::Client,
    base_url: Url,
    username: String,
    password: String,
}

#[derive(Debug, Clone)]
pub struct Calendar {
    pub url: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Event {
    pub uid: String,
    pub summary: String,
    pub location: Option<String>,
    pub description: Option<String>,
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
}

impl CalDavClient {
    pub async fn new(base_url: &str, username: &str, password: &str) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        let base_url = Url::parse(base_url)?;

        Ok(Self {
            client,
            base_url,
            username: username.to_string(),
            password: password.to_string(),
        })
    }

    pub fn get_calendar_url(&self, calendar_id: &str) -> String {
        let base_str = self.base_url.as_str();
        if base_str.ends_with(&format!("{}/", calendar_id)) || base_str.ends_with(calendar_id) {
            return base_str.to_string();
        }

        self.base_url
            .join(&format!("{}/", calendar_id))
            .map(|u| u.to_string())
            .unwrap_or_else(|_| format!("{}{}/", base_str, calendar_id))
    }

    pub async fn calendar_exists(&self, calendar_id: &str) -> Result<bool> {
        let url = self.get_calendar_url(calendar_id);
        let resp = self.request("PROPFIND", &url, Some(r#"<?xml version="1.0"?><d:propfind xmlns:d="DAV:"><d:prop><d:displayname/></d:prop></d:propfind>"#))
            .header("Depth", "0")
            .send()
            .await?;

        Ok(resp.status().is_success())
    }

    pub async fn create_calendar(&self, calendar_id: &str, display_name: &str) -> Result<Calendar> {
        let url = self.get_calendar_url(calendar_id);

        let body = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
            <mkcalendar xmlns="urn:ietf:params:xml:ns:caldav" xmlns:d="DAV:">
                <d:set>
                    <d:prop>
                        <d:displayname>{}</d:displayname>
                        <c:supported-calendar-component-set xmlns:c="urn:ietf:params:xml:ns:caldav">
                            <c:comp name="VEVENT"/>
                        </c:supported-calendar-component-set>
                    </d:prop>
                </d:set>
            </mkcalendar>"#,
            display_name
        );

        let resp = self.request("MKCALENDAR", &url, Some(&body)).send().await?;

        match resp.status().as_u16() {
            201 | 200 => {
                println!("  {} Calendar '{}' created", "✓".green(), calendar_id);
                Ok(Calendar {
                    url,
                    name: display_name.to_string(),
                })
            }
            405 => {
                anyhow::bail!(
                    "Server doesn't allow calendar creation via MKCALENDAR. Create calendar '{}' manually in web interface",
                    calendar_id
                )
            }
            403 => {
                anyhow::bail!(
                    "Forbidden: no permission to create calendar '{}'",
                    calendar_id
                )
            }
            _ => {
                anyhow::bail!("Failed to create calendar: HTTP {}", resp.status())
            }
        }
    }

    pub async fn delete_calendar(&self, calendar_id: &str) -> Result<()> {
        let url = self.get_calendar_url(calendar_id);
        let _ = self.request("DELETE", &url, None).send().await?;
        Ok(())
    }

    pub async fn put_event(&self, calendar_id: &str, event: &Event) -> Result<()> {
        let cal_url = self.get_calendar_url(calendar_id);
        let event_url = format!("{}osa2cal-{}.ics", cal_url, event.uid);

        let ical_data = self.event_to_ical(event);

        let resp = self
            .request("PUT", &event_url, Some(&ical_data))
            .header("Content-Type", "text/calendar; charset=utf-8")
            .send()
            .await?;

        if !resp.status().is_success() {
            anyhow::bail!("Failed to create event: HTTP {}", resp.status());
        }

        Ok(())
    }

    fn event_to_ical(&self, event: &Event) -> String {
        let now = Utc::now().format("%Y%m%dT%H%M%SZ");
        let start = event.start.format("%Y%m%dT%H%M%S");
        let end = event.end.format("%Y%m%dT%H%M%S");

        let mut ical = format!(
            r#"BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//osa2cal//College Schedule//EN
CALSCALE:GREGORIAN
BEGIN:VEVENT
UID:{}
DTSTAMP:{}
DTSTART;TZID=Europe/Moscow:{}
DTEND;TZID=Europe/Moscow:{}
SUMMARY:{}
"#,
            event.uid,
            now,
            start,
            end,
            escape_ical(&event.summary)
        );

        if let Some(loc) = &event.location {
            ical.push_str(&format!("LOCATION:{}\n", escape_ical(loc)));
        }
        if let Some(desc) = &event.description {
            ical.push_str(&format!("DESCRIPTION:{}\n", escape_ical(desc)));
        }

        ical.push_str("END:VEVENT\nEND:VCALENDAR");
        ical
    }

    fn request(&self, method: &str, url: &str, body: Option<&str>) -> reqwest::RequestBuilder {
        let mut req = self
            .client
            .request(reqwest::Method::from_bytes(method.as_bytes()).unwrap(), url)
            .basic_auth(&self.username, Some(&self.password));

        if let Some(b) = body {
            req = req.body(b.to_string());
        }

        req
    }
}

fn escape_ical(text: &str) -> String {
    text.replace("\\", "\\\\")
        .replace(";", "\\;")
        .replace(",", "\\,")
        .replace("\n", "\\n")
}
