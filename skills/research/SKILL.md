---
name: research
description: Perform web-based research on any topic and return a structured research brief. Spawns a scout agent in the background that uses browser automation to gather information.
license: Apache-2.0
compatibility: Requires g3 binary in PATH. WebDriver (Safari or Chrome) recommended for best results.
metadata:
  author: g3
  version: "2.0"
---

# Research Skill

Perform asynchronous web research without blocking your current work. Research runs in the background and results are saved to disk.

## Quick Start

```bash
# 1. Create research directory and status file
RESEARCH_ID="research_$(date +%s)_$(head -c 3 /dev/urandom | xxd -p)"
mkdir -p ".g3/research/$RESEARCH_ID"
echo '{"id":"'$RESEARCH_ID'","status":"running","query":"YOUR QUERY"}' > ".g3/research/$RESEARCH_ID/status.json"

# 2. Start research in background
background_process("research-topic", "g3 --agent scout --new-session --quiet 'Your research question' > .g3/research/$RESEARCH_ID/report.md 2>&1 && sed -i '' 's/running/complete/' .g3/research/$RESEARCH_ID/status.json || sed -i '' 's/running/failed/' .g3/research/$RESEARCH_ID/status.json")

# 3. Check status
cat .g3/research/$RESEARCH_ID/status.json

# 4. Read report when complete
read_file(".g3/research/$RESEARCH_ID/report.md")
```

## Step-by-Step Instructions

### 1. Generate a Unique Research ID

Use shell to create a unique ID and directory:

```bash
shell("RESEARCH_ID=\"research_$(date +%s)_$(head -c 3 /dev/urandom | xxd -p)\" && mkdir -p \".g3/research/$RESEARCH_ID\" && echo $RESEARCH_ID")
```

Save the returned ID for later use.

### 2. Write Initial Status File

```bash
shell("echo '{\"id\":\"<RESEARCH_ID>\",\"status\":\"running\",\"query\":\"<YOUR_QUERY>\",\"started_at\":\"'$(date -u +%Y-%m-%dT%H:%M:%SZ)'\"}' > .g3/research/<RESEARCH_ID>/status.json")
```

### 3. Start the Scout Agent

Use `background_process` to run the scout agent (NEVER use blocking `shell`):

```bash
background_process("research-<topic>", "g3 --agent scout --new-session --quiet '<Your detailed research question>' > .g3/research/<RESEARCH_ID>/report.md 2>&1; if [ $? -eq 0 ]; then sed -i '' 's/running/complete/' .g3/research/<RESEARCH_ID>/status.json; else sed -i '' 's/running/failed/' .g3/research/<RESEARCH_ID>/status.json; fi")
```

**Important flags:**
- `--agent scout` - Uses the scout agent optimized for web research
- `--new-session` - Starts a fresh session
- `--quiet` - Reduces UI noise in output

### 4. Check Research Status

```bash
shell("cat .g3/research/<RESEARCH_ID>/status.json")
```

Status values:
- `running` - Research in progress
- `complete` - Report ready to read  
- `failed` - Error occurred

### 5. Read the Report

Once status is `complete`:

```bash
read_file(".g3/research/<RESEARCH_ID>/report.md")
```

## Directory Structure

```
.g3/research/
└── research_1738700000_a1b2c3/
    ├── status.json      # Machine-readable status
    └── report.md        # The research brief (when complete)
```

## Example: Complete Workflow

```bash
# Step 1: Create research task
shell("RESEARCH_ID=\"research_$(date +%s)_$(head -c 3 /dev/urandom | xxd -p)\" && mkdir -p \".g3/research/$RESEARCH_ID\" && echo '{\"id\":\"'$RESEARCH_ID'\",\"status\":\"running\",\"query\":\"Rust async runtimes comparison\"}' > \".g3/research/$RESEARCH_ID/status.json\" && echo $RESEARCH_ID")
# Returns: research_1738700000_a1b2c3

# Step 2: Start scout in background  
background_process("research-rust-async", "g3 --agent scout --new-session --quiet 'Compare Tokio vs async-std vs smol for Rust async runtimes. Include performance, ecosystem, and ease of use.' > .g3/research/research_1738700000_a1b2c3/report.md 2>&1; [ $? -eq 0 ] && sed -i '' 's/running/complete/' .g3/research/research_1738700000_a1b2c3/status.json || sed -i '' 's/running/failed/' .g3/research/research_1738700000_a1b2c3/status.json")

# Step 3: Continue other work...
shell("cargo check")

# Step 4: Check if done
shell("cat .g3/research/research_1738700000_a1b2c3/status.json")

# Step 5: Read report
read_file(".g3/research/research_1738700000_a1b2c3/report.md")
```

## Listing All Research Tasks

```bash
shell("for f in .g3/research/*/status.json; do cat \"$f\" 2>/dev/null; echo; done")
```

## Best Practices

1. **Always use `background_process`** - Never run research with blocking `shell`
2. **Be specific** - Narrow queries get better results faster
3. **Read selectively** - Only load reports into context when you need them
4. **Check status first** - Don't try to read reports that aren't complete yet

## Troubleshooting

### Research takes too long
- Try a more specific query
- Complex topics may take 1-2 minutes

### WebDriver not available  
- Research will still work but may have limited web access
- The scout agent will fall back to shell-based methods

### Report is empty or failed
- Check status.json for the status
- Look at the report.md file for any error output
- The query may be too broad or the topic too obscure
