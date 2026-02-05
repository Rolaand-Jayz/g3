---
name: research
description: Perform web-based research on any topic and return a structured research brief. Spawns a scout agent in the background that uses browser automation to gather information.
license: Apache-2.0
compatibility: Requires g3 binary in PATH. WebDriver (Safari or Chrome) recommended for best results.
metadata:
  author: g3
  version: "1.0"
---

# Research Skill

Perform asynchronous web research without blocking your current work. Research runs in the background and saves results to disk for you to read when ready.

## Quick Start

```bash
# Start research (ALWAYS use background_process, never blocking shell)
background_process("research-<topic>", ".g3/bin/g3-research 'Your research question here'")

# Check status
shell(".g3/bin/g3-research --status <research-id>")
# Or list all:
shell(".g3/bin/g3-research --list")

# Read the report when complete
read_file(".g3/research/<research-id>/report.md")
```

## How It Works

1. **Start research** - The `g3-research` script spawns a scout agent that performs web research
2. **Background execution** - Research runs asynchronously; you can continue other work
3. **Filesystem handoff** - Results are written to `.g3/research/<id>/` with machine-readable status
4. **Read when ready** - Use `read_file` to load the report into context only when needed

## Directory Structure

```
.g3/research/
├── research_1738700000_a1b2c3/
│   ├── status.json      # Machine-readable status
│   └── report.md        # The research brief (when complete)
└── research_1738700100_d4e5f6/
    ├── status.json
    └── report.md
```

## status.json Schema

```json
{
  "id": "research_1738700000_a1b2c3",
  "query": "What are the best Rust async runtimes?",
  "status": "complete",
  "started_at": "2026-02-04T12:00:00Z",
  "completed_at": "2026-02-04T12:01:30Z",
  "report_path": ".g3/research/research_1738700000_a1b2c3/report.md",
  "error": null
}
```

**Status values:**
- `running` - Research in progress
- `complete` - Report ready to read
- `failed` - Error occurred (check `error` field)

## Commands

### Start Research

```bash
.g3/bin/g3-research "<query>"
```

Outputs the research ID and path on success. **Always run via `background_process`**, not `shell`.

### Check Status

```bash
# Check specific research
.g3/bin/g3-research --status <research-id>

# List all research tasks
.g3/bin/g3-research --list
```

Outputs JSON for machine parsing.

### Read Report

Once status is `complete`, read the report:

```bash
read_file(".g3/research/<research-id>/report.md")
```

**Tip:** If the report is large, use partial reads:
```bash
read_file(".g3/research/<id>/report.md", start=0, end=2000)
```

## Example Workflow

```
# 1. Start research on async runtimes
background_process("research-async", ".g3/bin/g3-research 'Compare Tokio vs async-std vs smol for Rust async runtimes'")

# 2. Continue with other work while research runs...
shell("cargo check")

# 3. Check if research is done
shell(".g3/bin/g3-research --list")

# 4. Read the report
read_file(".g3/research/research_1738700000_abc123/report.md")
```

## Best Practices

1. **Always use `background_process`** - Never run research with blocking `shell`
2. **Be specific** - Narrow queries get better results faster
3. **Read selectively** - Only load reports into context when you need them
4. **Check status first** - Don't try to read reports that aren't complete

## Troubleshooting

### Research takes too long
- Try a more specific query
- Complex topics may take 1-2 minutes

### WebDriver not available
- Research will still work but may have limited web access
- Install Safari WebDriver or Chrome for best results

### Report is empty or failed
- Check `status.json` for error details
- The query may be too broad or the topic too obscure

## Notes

- Research results accumulate in `.g3/research/` - they are not auto-cleaned
- Each research task gets a unique ID based on timestamp
- Multiple concurrent research tasks are supported
