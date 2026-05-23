# Service Configuration Guide

## Overview

fcast-android-sender supports two media services:

| Service | Description | Default mode |
|---------|-------------|-------------|
| **gst-pop** | GStreamer daemon for pipeline management | Android Service |
| **Migration** | In-process media engine | Embedded |

## Configuration file

Service settings are stored in `backend.json` alongside the existing backend configuration:

```json
{
  "kind": "migration",
  "gstpop_url": "ws://127.0.0.1:9000",
  "gstpop_api_key": null,
  "gstpop_pipeline_id": "0",
  "gstpop_service": {
    "enabled": true,
    "auto_start": true,
    "mode": "android-service"
  },
  "migration_service": {
    "enabled": true,
    "auto_start": true,
    "mode": "embedded"
  },
  "auto_start_services": true,
  "service_mode": "embedded"
}
```

## Service modes

| Mode | Description |
|------|-------------|
| `embedded` | Run inside the app process |
| `android-service` | Managed Android foreground Service |
| `external` | Connect to a user-supplied daemon |

## UI access

Open **Settings > Service Configuration** to toggle services and change modes at runtime.

## Running without services

The app can run with all services disabled. The UI will show a "No service running" notice with a link to the configuration page. Media features (casting, mixing) are unavailable until at least one service is started.
