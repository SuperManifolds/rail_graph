# CSV Import Guide

RailGraph supports importing railway timetables and infrastructure from CSV files. The importer features intelligent column detection, multiple import modes, and support for both simple and complex multi-line formats.

## Table of Contents
- [Column Types](#column-types)
- [Import Modes](#import-modes)
- [Special Markers](#special-markers)
- [Pattern Repeat Feature](#pattern-repeat-feature)
- [Examples](#examples)
- [Auto-Detection](#auto-detection)
- [Best Practices](#best-practices)

## Column Types

RailGraph recognizes the following column types:

### Required Columns

#### Station Name
The name of the station. This is the only required column for any CSV import.
- **Header keywords:** "station", "stop", "name"
- **Format:** Any text
- **Example:** `Trondheim`, `Oslo S`, `Stavanger`

### Infrastructure Columns

These columns define the physical railway infrastructure:

#### Platform
Platform number or name at the station.
- **Header keywords:** "platform", "plat"
- **Format:** Number (e.g., `1`, `2`, `3`) or name (e.g., `A`, `Platform 1`)
- **Example:** `3` (creates platforms 1, 2, and 3)
- **Note:** Numeric values create all platforms up to that number

#### Track Number
Which track the train uses on the next segment.
- **Header keywords:** "track"
- **Format:** Number (1, 2, 3, etc.)
- **Example:** `2` (train uses track 2)
- **Note:** Used for multi-track sections. Track 1 = main track.

#### Track Distance
Distance from the previous station in kilometers.
- **Header keywords:** "distance", "km"
- **Format:** Decimal number
- **Example:** `6.63`, `12.5`
- **Note:** This is a global column (not per-line in multi-line format)

#### Distance Offset
Cumulative distance from the start of the line in kilometers.
- **Format:** Decimal number
- **Example:** `0`, `6.63`, `13.52` (automatically converted to inter-node distances: 6.63, 6.89)
- **Note:** This is a global column (not per-line in multi-line format)
- **Usage:** Useful when you have total distance markers but not segment distances. The importer automatically calculates the distance between each pair of consecutive stations.

### Timing Columns

These columns define when trains arrive and depart. You must use **one** of these timing methods per line:

#### Arrival Time
Absolute time when train arrives at station.
- **Header keywords:** "arr", "arrival"
- **Format:** `H:MM:SS` or `HH:MM:SS`
- **Example:** `5:30:00`, `14:25:00`
- **Detection:** Times ≥ 4 hours are auto-detected as Arrival/Departure

#### Departure Time
Absolute time when train departs from station.
- **Header keywords:** "dep", "departure"
- **Format:** `H:MM:SS` or `HH:MM:SS`
- **Example:** `5:35:00`, `14:30:00`
- **Note:** Usually paired with Arrival Time

#### Offset
Cumulative time from the first station (relative time).
- **Header keywords:** "offset", "time"
- **Format:** `H:MM:SS`
- **Example:** `0:00:00` (first station), `0:15:30` (15.5 minutes later)
- **Detection:** Monotonically increasing times starting from 0:00:00

#### Travel Time
Duration of travel from the previous station.
- **Header keywords:** "travel", "duration"
- **Format:** `H:MM:SS`, `MM:SS`
- **Example:** `0:05:30`
- **Detection:** Short durations (< 1 hour, > 2 minutes)

#### Wait Time
How long the train waits at this station (dwell time).
- **Header keywords:** "wait", "dwell"
- **Format:** `H:MM:SS`, `MM:SS`
- **Example:** `0:02:00`,
- **Priority:**
  1. Passing loops always get 0 wait time
  2. Departure - Arrival (if both present)
  3. Wait Time column value
  4. Default wait time (30 seconds)

### Other Columns

#### Skip
Explicitly ignore this column during import.
- **Format:** Any (will be ignored)
- **Use case:** Extra notes, comments, or data you don't want imported

## Import Modes

RailGraph supports three different import modes:

### 1. Normal Mode (Default)
Creates both infrastructure (stations, tracks) and lines (routes, timetables).

**Use when:**
- Starting a new project
- Adding new lines to an existing network
- Infrastructure doesn't exist yet

**Result:**
- New stations and tracks are created
- Lines with timetables are created
- Track directions are automatically configured

### 2. Infrastructure-Only Mode
Creates only the railway infrastructure without any lines or timetables.

**Trigger:** CSV has only infrastructure columns (no timing columns)

**Use when:**
- Building network infrastructure first
- Importing track layouts before scheduling
- Creating a base network for multiple lines

**Result:**
- Stations, junctions, and tracks are created
- Platform and track configurations are set
- No lines or routes are created

**Example CSV:**
```csv
Station,Distance,Platform,Track
Oslo S,0,8,
Lillestrøm,21.4,2,2
Eidsvoll,43.2,2,
```

### 3. Pathfinding Mode ("Don't create new infrastructure")
Uses existing infrastructure to create routes via pathfinding.

**Trigger:** Check the "Don't create new infrastructure" option in the import dialog

**Use when:**
- Adding new lines to existing infrastructure
- Creating alternative routes on the same network
- Testing different timetables on existing tracks

**Requirements:**
- All stations in CSV must already exist
- Paths must exist between consecutive stations
- Junction markers (J) must match existing junctions

**Result:**
- No new stations or tracks created
- Routes are found via pathfinding
- Lines are created using existing infrastructure

**Error handling:**
- Import fails if station not found
- Import fails if no path exists between stations
- Clear error messages indicate which station/path failed

## Special Markers

### Passing Loop Marker: `(P)`
Indicates a station is a passing loop (trains don't stop).

**Syntax:** Add `(P)` to the end of the station name

**Effects:**
- Station is marked as passing loop
- Wait time is automatically set to 0
- Train passes through without stopping

**Example:**
```csv
Station,Offset
Larvik,0:00:00
Kjose (P),0:15:00
Porsgrunn,0:30:00
```

### Junction Marker: `(J)`
Indicates a junction where tracks split or merge.

**Syntax:** Add `(J)` to the end of the station name

**Effects:**
- Creates a junction node instead of a station
- Junction can connect to multiple tracks
- Used for track routing and switching

**Example:**
```csv
Station,Offset
Trondheim,0:00:00
Leangen,0:05:00
Ladalen (J),0:08:00
Stjørdal,0:15:00
```

**Note:** In pathfinding mode, junctions are identified by both name and connection to the previous station.

## Pattern Repeat Feature

The pattern repeat feature automatically detects when columns follow a repeating pattern, allowing you to import multiple lines in a single CSV.

### How It Works

1. **Pattern Detection:** The importer analyzes column types after the station column
2. **Pattern Matching:** Looks for repeating sequences (e.g., Arr, Dep, Arr, Dep)
3. **Grouping:** Groups columns into lines based on the pattern
4. **Line Names:** Extracts line names from column headers (e.g., "R70", "Line 1")

### Pattern Requirements

- Minimum pattern length: 2 columns
- Pattern must repeat at least twice
- Column types must match exactly in each repetition
- Applies to everything after the station column (except global columns like Distance)

### Example: Two Lines

```csv
Station,R70 Arr,R70 Dep,R71 Arr,R71 Dep
Oslo S,5:00:00,5:02:00,5:30:00,5:32:00
Lillestrøm,5:20:00,5:22:00,5:50:00,5:52:00
Eidsvoll,5:45:00,5:47:00,6:15:00,6:17:00
```

**Detected pattern:** Arrival, Departure (length 2)
- Group 0 (R70): Columns 1-2
- Group 1 (R71): Columns 3-4

### Example: Three Lines with Offset

```csv
Station,L1 Time,L1 Wait,L2 Time,L2 Wait,L3 Time,L3 Wait
Bergen,0:00:00,0:02:00,0:00:00,0:02:00,0:00:00,0:02:00
Arna,0:15:00,0:01:00,0:18:00,0:01:00,0:12:00,0:01:00
Voss,1:20:00,0:03:00,1:25:00,0:03:00,1:15:00,0:03:00
```

**Detected pattern:** Offset, Wait Time (length 2)
- Group 0 (L1): Columns 1-2
- Group 1 (L2): Columns 3-4
- Group 2 (L3): Columns 5-6

## Examples

### Example 1: Simple Single-Line (Offset Format)

Most common format for a single line using cumulative time.

```csv
Station,Distance,Offset,Wait,Platform,Track
Støren,6.63,0:00:00,,4,
Hovin,6.89,0:05:11,,2,
Lundamo,5.89,0:09:59,,2,
Ler,4.608,0:13:57,,2,
Melhus,5.42,0:22:21,,2,
Heimdal,2,0:29:41,,3,2
Trondheim,0.791,0:38:35,,3,2
```

**Creates:**
- One line with 7 stations
- Platform configurations (Trondheim gets 3 platforms)
- Track 2 from Heimdal to Trondheim (double-track section)
- Routes with proper timing

### Example 2: Using Distance Offset (Cumulative Distance)

Same line as Example 1, but using cumulative distance markers instead of segment distances.

```csv
Station,Distance Offset,Offset,Wait,Platform,Track
Støren,0,0:00:00,,4,
Hovin,6.63,0:05:11,,2,
Lundamo,13.52,0:09:59,,2,
Ler,18.128,0:13:57,,2,
Melhus,23.548,0:22:21,,2,
Heimdal,25.548,0:29:41,,3,2
Trondheim,26.339,0:38:35,,3,2
```

**Creates:**
- Same infrastructure as Example 1
- Distances automatically converted: 0→6.63 km, 6.63→13.52 km (=6.89 km), etc.
- Useful when you have kilometer markers along the route

### Example 3: Infrastructure-Only Import

Create network infrastructure without timetables.

```csv
Station,Distance,Platform,Track
Drammen,0,4,
Hokksund,18,2,
Kongsberg,24,2,
Nordagutu,42,2,2
Hjuksebø (P),15,0,2
Notodden,12,2,
```

**Creates:**
- 6 stations (including 1 passing loop)
- Platform counts configured
- Double-track from Nordagutu to Notodden
- No lines or timetables

### Example 4: Arrival/Departure Times

Using absolute arrival and departure times.

```csv
Station,Arrival,Departure,Platform
Oslo S,5:00:00,5:05:00,8
Lillestrøm,5:25:00,5:27:00,2
Dal,5:45:00,5:46:00,1
Eidsvoll,6:00:00,6:02:00,2
Hamar,6:40:00,6:42:00,3
```

**Creates:**
- Line using manual schedule mode (single departure at 5:00)
- Wait times calculated from Departure - Arrival
- First station departs at 5:00 (waits 5 minutes)

### Example 5: Travel Time Format

Using incremental travel times between stations.

```csv
Station,Travel Time,Wait Time
Trondheim,0:00:00,2min
Heimdal,10min,1min
Ranheim,8min,30s
Hell,6min,2min
Stjørdal,15min,
```

**Creates:**
- Line with travel durations between stations
- Custom wait times at each station
- Automatic schedule generation

### Example 6: Multi-Line Grouped Format

Import multiple lines in one CSV using pattern repeat.

```csv
Station,Distance,R70 Offset,R70 Wait,R71 Offset,R71 Wait
Trondheim,0,0:00:00,0:02:00,0:00:00,0:03:00
Heimdal,8,0:10:00,0:01:00,0:12:00,0:01:00
Ranheim,12,0:18:00,0:01:00,0:21:00,0:01:00
Hell,15,0:27:00,0:02:00,0:32:00,0:02:00
Stjørdal,18,0:42:00,,0:48:00,
```

**Creates:**
- Two lines (R70 and R71)
- Shared infrastructure (same stations and tracks)
- Different timetables for each line
- Distance column shared by both lines

### Example 7: Mixed Format with Junctions

Complex example with junctions, passing loops, and double-track.

```csv
Station,Distance,Offset,Platform,Track
Oslo S,0,0:00:00,8,
Nationaltheatret,1.5,0:03:00,2,2
Skøyen,3.2,0:08:00,2,2
Lysaker,2.1,0:11:00,2,2
Sandvika,5.8,0:18:00,4,2
Asker Junction (J),8.2,0:26:00,,2
Heggedal (P),6.5,0:32:00,,1
Drammen,12.3,0:45:00,4,1
```

**Creates:**
- 8 stations (1 junction, 1 passing loop)
- Double-track Oslo S to Asker Junction
- Single-track after junction with passing loop
- Platform configurations

### Example 8: Pathfinding Mode

Using existing infrastructure (must check "Don't create new infrastructure").

```csv
Station,Offset,Wait
Oslo S,0:00:00,0:02:00
Lillestrøm,0:20:00,0:01:00
Eidsvoll,0:45:00,
```

**Requirements:**
- All three stations must already exist in the infrastructure
- A path must exist from Oslo S → Lillestrøm → Eidsvoll

**Creates:**
- One line following the existing track path
- No new infrastructure
- Route via pathfinding algorithm

## Auto-Detection

RailGraph automatically detects column types using headers and data samples.

### Header Detection

Column headers are matched against keywords (case-insensitive):

| Keywords | Detected Type |
|----------|---------------|
| station, stop, name | Station Name |
| platform, plat | Platform |
| distance, km | Track Distance |
| track | Track Number |
| arr, arrival | Arrival Time |
| dep, departure | Departure Time |
| travel, duration | Travel Time |
| wait, dwell | Wait Time |
| offset, time | Offset |

### Data-Based Detection

When headers are ambiguous, the importer analyzes sample data:

**Time Detection:**
- Times ≥ 4 hours → Arrival/Departure Time
- Monotonically increasing from 0:00:00 → Offset
- Short durations (< 1 hour, > 2 min) → Travel Time
- Very short durations (< 2 min) → Wait Time

**Numeric Detection:**
- Small integers (1-10) → Track Number or Platform
- Decimal numbers → Track Distance

**Context-Aware:**
- Arrival column followed by time → Departure Time
- Previous column is station → likely timing column

### Manual Override

You can always manually adjust column types in the import dialog:
1. Import your CSV
2. Review detected column types
3. Click any column type to change it
4. Adjust line grouping if needed
5. Complete import

## Best Practices

### CSV Format

1. **Use headers:** Header row helps auto-detection
2. **Consistent formatting:** Keep time formats consistent
3. **Empty values:** Leave cells blank rather than using 0 or "-"
4. **UTF-8 encoding:** Use UTF-8 for special characters (å, ä, ö, etc.)

### Time Format

1. **Prefer Offset:** Simplest for single lines, easiest to edit
2. **Use Arrival/Departure:** For exact timetables from real schedules
3. **Travel Time:** Good for relative timing, easy to adjust distances
4. **Consistency:** Don't mix time formats in one line

### Infrastructure

1. **Platform numbers:** Use numbers (1, 2, 3) for automatic creation
2. **Track numbers:** Essential for multi-track sections
3. **Distance:** Optional but useful for visualization
4. **Markers:** Use (P) for passing loops, (J) for junctions

### Multi-Line Import

1. **Consistent patterns:** Keep column patterns identical
2. **Descriptive headers:** Use line names/numbers in headers
3. **Shared infrastructure:** Infrastructure columns (Distance, Track) are global
4. **Test individually:** Test each line separately before multi-line import

### Workflow

1. **Infrastructure first:** Consider infrastructure-only import for complex networks
2. **Build gradually:** Start with main line, add branches later
3. **Test imports:** Use small test files to verify format
4. **Pathfinding later:** Use pathfinding mode for additional lines on existing infrastructure

### Common Pitfalls

- **Mixed time formats:** Using both Offset and Arrival/Departure in same line
- **Missing stations:** In pathfinding mode, ensure all stations exist
- **Disconnected tracks:** Ensure paths exist between consecutive stations
- **Inconsistent grouping:** Pattern repeat requires exact column type matches

## Example Files

See the `test-data/` directory for working examples:

- **R70.csv** - Single line with offset format (Norwegian rail)
- **infra.csv** - Infrastructure-only import
- **F6.csv, F7.csv, L7.csv** - Regional lines
- **R71.csv, R75.csv** - Regional express lines

All example files demonstrate real-world timetable formats and can be used as templates for your own imports.
