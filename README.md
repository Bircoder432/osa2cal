
# osa2cal

CLI utility to sync college schedules from OpenScheduleAPI to CalDAV calendars or ICS files.

(!) WARNING: This utility is in early development stage and may be very unstable
## Features

- **CalDAV Sync**: Upload schedules directly to CalDAV servers (Nextcloud, Baikal, etc.)
- **ICS Export**: Generate standard `.ics` files for manual import
- **Terminal View**: Display schedules in your terminal
- **Flexible Periods**: Sync today, tomorrow, week, month, or specific dates
- **Smart Calendar Management**: Use existing calendars or auto-create with `--force`

## Installation

```bash
cargo install --path .
```

Or build from source:

```bash
git clone https://github.com/bircoder432/osa2cal
cd osa2cal
cargo build --release
```

## Configuration

Create config at `~/.config/osa2cal/config.toml`:

```toml
api_url = "https://api.schedule.example.com"
default_group = 123
college_name = "My College"
calendar_name = "schedule"

caldav_url = "https://cal.example.com/dav.php/calendars/username/"
caldav_username = "username"
```

**Password** is set via environment variable (not stored in config):
```bash
export OSARS_CALDAV_PASSWORD="your_password"
```

## Usage

### List Available Data

```bash
# List colleges
osa2cal list colleges

# List campuses for college #1
osa2cal list campuses 1

# List groups for campus #5
osa2cal list groups 5
```

### View Schedule in Terminal

```bash
# Show this week (default)
osa2cal show

# Show today
osa2cal show --period today

# Show specific date
osa2cal show --period 2026-03-15
```

### Export to ICS File

```bash
# Export this month to file
osa2cal export --output schedule.ics --period month

# Export today only
osa2cal export --period today --output today.ics
```

### Sync to CalDAV

```bash
# Sync to calendar from config
osa2cal sync --period week

# Sync to specific calendar
osa2cal sync --period month --calendar-id myschedule

# Dry run (preview without uploading)
osa2cal sync --period today --dry-run

# Force create calendar if not exists (server must support MKCALENDAR)
osa2cal sync --period week --calendar-id newcal --force

# Recreate calendar (delete existing + create new)
osa2cal sync --period month --force --calendar-id schedule
```

## Period Options

| Period | Description |
|--------|-------------|
| `today` | Current day only |
| `tomorrow` | Next day |
| `week` | Current week (all 7 days) |
| `month` | Current + next week |
| `all` | Available weeks (previous, current, next) |
| `DD-MM-YYYY` | Specific date |

## CalDAV Server Compatibility

| Server | Auth | Auto-create | Notes |
|--------|------|-------------|-------|
| **Baikal** | Basic/Digest | Manual only | Use `--force` with Basic auth |
| **Nextcloud** | Basic | Yes | Standard paths |
| **Yandex** | Basic | No | Pre-create calendar |
| **iCloud** | App-specific | No | Use app-specific password |

## Troubleshooting

### 401 Unauthorized
Check password: `echo "$OSARS_CALDAV_PASSWORD"`

### 403 Forbidden (create calendar)
Server doesn't allow MKCALENDAR. Create calendar manually in web UI, then sync to it.

### 405 Method Not Allowed
Wrong CalDAV URL path. Verify with:
```bash
curl -u "user:pass" -X PROPFIND "https://cal.example.com/dav.php/calendars/user/"
```

### Empty schedule
API may be down or `?weekday=` parameter broken. Try:
```bash
osa2cal show --period today  # test minimal request
```

## Event Format

Generated events follow this structure:

- **Summary**: Lesson title (e.g., "Mathematics")
- **Location**: `College - Cabinet` (e.g., "ТКПСТ - Room 101")
- **Description**: Teacher name and group ID
- **UID**: Deterministic ID for deduplication (`osa2cal-{group}-{date}-{order}`)

## License

MIT
