# Nixe Maps Integration Contracts

This document specifies the integration protocols and examples for **Nixe Assistant** and **Nixe Glimpse**.

---

## 1. Nixe Assistant Integration

Nixe Assistant can access geographic and mapping capabilities either headlessly via CLI JSON or over D-Bus without launching the full user interface.

### Headless CLI API (`--api`)

#### 1.1 Location Search
```bash
nixe-maps --api search "Nashville"
```
Output:
```json
[
  {
    "name": "Nashville",
    "formatted_address": "Nashville, Davidson County, Tennessee, United States",
    "lat": 36.1627,
    "lon": -86.7816,
    "category": "city"
  }
]
```

#### 1.2 Distance & Travel Time Calculation
```bash
nixe-maps --api distance "36.1627,-86.7816" "35.1495,-90.0490" drive
```
Output:
```json
{
  "mode": "Drive",
  "distance_meters": 341500.0,
  "duration_seconds": 11400.0,
  "formatted_distance": "212.2 mi",
  "formatted_duration": "3 hr 10 min"
}
```

#### 1.3 Full Route
```bash
nixe-maps --api route "36.1627,-86.7816" "35.1495,-90.0490" drive
```

### Assistant Ability Manifest Example (`plugin.toml`)
```toml
manifest_version = 1
id = "maps"
version = "1.0.0"
description = "Maps and routing capabilities powered by Nixe Maps"
executable = "nixe-maps"
args = ["--api"]
tools = ["maps.search", "maps.distance", "maps.route"]

[[routes]]
pattern = "^(?i:show (?P<location>.+) on (?:the )?map)$"
exec = ["nixe-maps", "--search", "{location}"]

[[routes]]
pattern = "^(?i:how far is (?P<destination>.+))$"
tool = "maps.distance"

[[routes]]
pattern = "^(?i:get directions (?:to |from )?(?P<rest>.+))$"
exec = ["nixe-maps", "nixe-maps://route?{rest}"]
```

---

## 2. Nixe Glimpse Integration

Glimpse executes commands to launch Nixe Maps with coordinates, places, or routes pre-selected.

### Supported Commands
- `map Nashville` -> `nixe-maps "Nashville"`
- `map 35.1495,-90.0490` -> `nixe-maps "35.1495,-90.0490"`
- `route home to work` -> `nixe-maps "nixe-maps://route?start=home&end=work"`
- `geo:36.1627,-86.7816` -> Standard RFC 5870 URI handled by desktop launcher.

---

## 3. D-Bus Interface

When Nixe Maps is running, it registers a session bus service:
- **Bus Name:** `io.github.design_nexus.nixe.Maps`
- **Object Path:** `/io/github/design_nexus/nixe/Maps`
- **Interface:** `io.github.design_nexus.nixe.Maps`

### Methods:
1. `ShowLocation(f64 lat, f64 lon, String label)`: Centers the map on the coordinate, places a marker, and displays place details.
2. `Search(String query)`: Enters the search query and opens results.
3. `ShowRoute(f64 start_lat, f64 start_lon, f64 end_lat, f64 end_lon, String mode)`: Draws the route and opens the directions panel.
4. `GetDistance(f64 start_lat, f64 start_lon, f64 end_lat, f64 end_lon, String mode) -> (f64 distance_meters, f64 duration_seconds)`: Returns numeric metrics.
