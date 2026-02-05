//! SDLC Pipeline - Software Development Life Cycle maintenance pipeline
//!
//! Orchestrates a sequence of g3 agents to maintain and improve the codebase:
//! 1. euler   - Dependency graph and hotspots analysis
//! 2. breaker - Whitebox exploration and edge-case discovery
//! 3. hopper  - Deep testing and regression integrity
//! 4. fowler  - Refactoring to deduplicate and reduce complexity
//! 5. carmack - In-place rewriting for readability and concision
//! 6. lamport - Human-readable documentation and validation
//! 7. huffman - Semantic compression of memory

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Pipeline stage definition
#[derive(Debug, Clone)]
pub struct Stage {
    /// Agent name (e.g., "euler")
    pub name: &'static str,
    /// Human-readable description
    pub description: &'static str,
    /// What this agent focuses on
    pub focus: &'static str,
}

/// The ordered pipeline stages
pub static PIPELINE_STAGES: &[Stage] = &[
    Stage {
        name: "euler",
        description: "Dependency Analysis",
        focus: "dependency graph and hotspots",
    },
    Stage {
        name: "breaker",
        description: "Edge Case Discovery",
        focus: "whitebox exploration and failure cases",
    },
    Stage {
        name: "hopper",
        description: "Testing & Verification",
        focus: "deep testing and regression integrity",
    },
    Stage {
        name: "fowler",
        description: "Refactoring",
        focus: "deduplication and complexity reduction",
    },
    Stage {
        name: "carmack",
        description: "Code Polish",
        focus: "readability, modularity and concision",
    },
    Stage {
        name: "lamport",
        description: "Documentation",
        focus: "human-readable docs and validation",
    },
    Stage {
        name: "huffman",
        description: "Memory Compression",
        focus: "semantic compression to preserve signal",
    },
];

/// Status of a single stage execution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum StageStatus {
    /// Not yet started
    Pending,
    /// Currently running
    Running,
    /// Completed successfully
    Complete {
        duration_secs: u64,
        commits_processed: u32,
    },
    /// Failed with error
    Failed {
        error: String,
        attempts: u32,
    },
    /// Skipped (e.g., no new commits)
    Skipped { reason: String },
}

/// State of a single stage in the pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageState {
    /// Agent name
    pub name: String,
    /// Current status
    pub status: StageStatus,
    /// When this stage started (if running or complete)
    pub started_at: Option<DateTime<Utc>>,
    /// When this stage completed (if complete)
    pub completed_at: Option<DateTime<Utc>>,
    /// Commit hash when this stage last ran
    pub last_commit: Option<String>,
}

impl StageState {
    /// Create a new pending stage state
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            status: StageStatus::Pending,
            started_at: None,
            completed_at: None,
            last_commit: None,
        }
    }
}

/// The full pipeline state, persisted to disk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineState {
    /// Unique run identifier
    pub run_id: String,
    /// When this pipeline run started
    pub started_at: DateTime<Utc>,
    /// When this pipeline run completed (if complete)
    pub completed_at: Option<DateTime<Utc>>,
    /// Current stage index (0-based)
    pub current_stage: usize,
    /// State of each stage
    pub stages: Vec<StageState>,
    /// The commit cursor - commits up to this hash have been processed
    pub commit_cursor: Option<String>,
    /// Number of commits to process per run
    pub commits_per_run: u32,
    /// Git worktree session ID (for crash recovery)
    pub session_id: Option<String>,
}

impl PipelineState {
    /// Create a new pipeline state
    pub fn new(commits_per_run: u32) -> Self {
        let run_id = uuid::Uuid::new_v4().to_string()[..8].to_string();
        let stages = PIPELINE_STAGES
            .iter()
            .map(|s| StageState::new(s.name))
            .collect();

        Self {
            run_id,
            started_at: Utc::now(),
            completed_at: None,
            current_stage: 0,
            stages,
            commit_cursor: None,
            commits_per_run,
            session_id: None,
        }
    }

    /// Get the path to the pipeline state file
    pub fn state_path(repo_root: &Path) -> PathBuf {
        repo_root.join(".g3").join("sdlc").join("pipeline.json")
    }

    /// Get the path to the SDLC directory
    pub fn sdlc_dir(repo_root: &Path) -> PathBuf {
        repo_root.join(".g3").join("sdlc")
    }

    /// Load pipeline state from disk, or return None if not found
    pub fn load(repo_root: &Path) -> Result<Option<Self>> {
        let path = Self::state_path(repo_root);
        
        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&path)
            .context("Failed to read pipeline state")?;

        // Handle corrupted state gracefully
        match serde_json::from_str(&content) {
            Ok(state) => Ok(Some(state)),
            Err(e) => {
                eprintln!("⚠️  Pipeline state corrupted, starting fresh: {}", e);
                Ok(None)
            }
        }
    }

    /// Save pipeline state to disk
    pub fn save(&self, repo_root: &Path) -> Result<()> {
        let dir = Self::sdlc_dir(repo_root);
        fs::create_dir_all(&dir)
            .context("Failed to create analysis/sdlc directory")?;

        let path = Self::state_path(repo_root);
        let json = serde_json::to_string_pretty(self)
            .context("Failed to serialize pipeline state")?;
        
        fs::write(&path, json)
            .context("Failed to write pipeline state")?;

        Ok(())
    }

    /// Delete pipeline state from disk
    pub fn delete(repo_root: &Path) -> Result<()> {
        let path = Self::state_path(repo_root);
        if path.exists() {
            fs::remove_file(&path)
                .context("Failed to delete pipeline state")?;
        }
        Ok(())
    }

    /// Check if the pipeline is complete
    pub fn is_complete(&self) -> bool {
        self.stages.iter().all(|s| {
            matches!(
                s.status,
                StageStatus::Complete { .. } | StageStatus::Skipped { .. }
            )
        })
    }

    /// Check if the pipeline has any failures
    pub fn has_failures(&self) -> bool {
        self.stages.iter().any(|s| matches!(s.status, StageStatus::Failed { .. }))
    }

    /// Get the current stage definition
    pub fn current_stage_def(&self) -> Option<&'static Stage> {
        PIPELINE_STAGES.get(self.current_stage)
    }

    /// Mark the current stage as running
    pub fn mark_running(&mut self) {
        if let Some(stage) = self.stages.get_mut(self.current_stage) {
            stage.status = StageStatus::Running;
            stage.started_at = Some(Utc::now());
        }
    }

    /// Mark the current stage as complete and advance
    pub fn mark_complete(&mut self, duration_secs: u64, commits_processed: u32, commit_hash: &str) {
        if let Some(stage) = self.stages.get_mut(self.current_stage) {
            stage.status = StageStatus::Complete {
                duration_secs,
                commits_processed,
            };
            stage.completed_at = Some(Utc::now());
            stage.last_commit = Some(commit_hash.to_string());
        }
        
        // Advance to next stage
        if self.current_stage < PIPELINE_STAGES.len() - 1 {
            self.current_stage += 1;
        } else {
            // Pipeline complete
            self.completed_at = Some(Utc::now());
        }
        
        // Update cursor
        self.commit_cursor = Some(commit_hash.to_string());
    }

    /// Mark the current stage as failed
    pub fn mark_failed(&mut self, error: &str) {
        if let Some(stage) = self.stages.get_mut(self.current_stage) {
            let attempts = match &stage.status {
                StageStatus::Failed { attempts, .. } => attempts + 1,
                _ => 1,
            };
            stage.status = StageStatus::Failed {
                error: error.to_string(),
                attempts,
            };
        }
    }

    /// Mark the current stage as skipped
    #[allow(dead_code)]
    pub fn mark_skipped(&mut self, reason: &str) {
        if let Some(stage) = self.stages.get_mut(self.current_stage) {
            stage.status = StageStatus::Skipped {
                reason: reason.to_string(),
            };
            stage.completed_at = Some(Utc::now());
        }
        
        // Advance to next stage
        if self.current_stage < PIPELINE_STAGES.len() - 1 {
            self.current_stage += 1;
        } else {
            self.completed_at = Some(Utc::now());
        }
    }

    /// Retry the current failed stage
    #[allow(dead_code)]
    pub fn retry_stage(&mut self) -> Result<()> {
        if let Some(stage) = self.stages.get_mut(self.current_stage) {
            match &stage.status {
                StageStatus::Failed { .. } => {
                    // Keep the attempt count but reset to pending
                    stage.status = StageStatus::Pending;
                    stage.started_at = None;
                    Ok(())
                }
                _ => bail!("Stage '{}' is not in failed state", stage.name),
            }
        } else {
            bail!("Invalid current stage index")
        }
    }

    /// Find the first incomplete stage (for resumption)
    pub fn find_resume_point(&self) -> usize {
        for (i, stage) in self.stages.iter().enumerate() {
            match &stage.status {
                StageStatus::Pending | StageStatus::Running | StageStatus::Failed { .. } => {
                    return i;
                }
                _ => continue,
            }
        }
        // All complete
        self.stages.len()
    }

    /// Resume from the first incomplete stage
    pub fn resume(&mut self) {
        self.current_stage = self.find_resume_point();
        
        // If current stage was running (crashed), reset to pending
        if let Some(stage) = self.stages.get_mut(self.current_stage) {
            if matches!(stage.status, StageStatus::Running) {
                stage.status = StageStatus::Pending;
                stage.started_at = None;
            }
        }
    }
}

/// Get a stage by name
#[allow(dead_code)]
pub fn get_stage(name: &str) -> Option<&'static Stage> {
    PIPELINE_STAGES.iter().find(|s| s.name == name)
}

/// Get stage index by name
#[allow(dead_code)]
pub fn get_stage_index(name: &str) -> Option<usize> {
    PIPELINE_STAGES.iter().position(|s| s.name == name)
}

/// Display the pipeline with current stage highlighted
pub fn display_pipeline(state: &PipelineState) {
    println!();
    println!("\x1b[1m┌─────────────────────────────────────────────────────────────┐\x1b[0m");
    println!("\x1b[1m│                    SDLC Pipeline                            │\x1b[0m");
    println!("\x1b[1m├─────────────────────────────────────────────────────────────┤\x1b[0m");
    
    for (i, stage_def) in PIPELINE_STAGES.iter().enumerate() {
        let stage_state = &state.stages[i];
        let is_current = i == state.current_stage;
        
        let (icon, color) = match &stage_state.status {
            StageStatus::Pending => ("○", "\x1b[90m"),      // Gray
            StageStatus::Running => ("◉", "\x1b[33m"),      // Yellow
            StageStatus::Complete { .. } => ("✓", "\x1b[32m"), // Green
            StageStatus::Failed { .. } => ("✗", "\x1b[31m"),   // Red
            StageStatus::Skipped { .. } => ("⊘", "\x1b[90m"),  // Gray
        };
        
        let highlight = if is_current { "\x1b[1m" } else { "" };
        let reset = "\x1b[0m";
        
        // Pad to fixed width
        let padded = format!("{:<57}", format!("{} {:<10} - {}", icon, stage_def.name, stage_def.description));
        
        if is_current {
            println!("│ {}{}{}► {}{}│", color, highlight, reset, padded, reset);
        } else {
            println!("│ {}{}{}  {}│", color, highlight, padded, reset);
        }
    }
    
    println!("\x1b[1m└─────────────────────────────────────────────────────────────┘\x1b[0m");
    println!();
}

/// Display a compact single-line status for the current stage
pub fn display_current_stage(state: &PipelineState) {
    if let Some(stage) = state.current_stage_def() {
        println!(
            "\x1b[1;32msdlc:\x1b[0m stage {}/{} \x1b[1m{}\x1b[0m - {}",
            state.current_stage + 1,
            PIPELINE_STAGES.len(),
            stage.name,
            stage.focus
        );
    }
}

/// Generate a summary of the pipeline run
pub fn generate_summary(state: &PipelineState) -> String {
    let mut summary = String::new();
    
    summary.push_str("\n## SDLC Pipeline Summary\n\n");
    summary.push_str(&format!("**Run ID:** {}\n", state.run_id));
    summary.push_str(&format!("**Started:** {}\n", state.started_at.format("%Y-%m-%d %H:%M:%S UTC")));
    
    if let Some(completed) = state.completed_at {
        summary.push_str(&format!("**Completed:** {}\n", completed.format("%Y-%m-%d %H:%M:%S UTC")));
        let duration = completed.signed_duration_since(state.started_at);
        summary.push_str(&format!("**Total Duration:** {}\n", format_duration(duration.num_seconds() as u64)));
    }
    
    summary.push_str("\n### Stage Results\n\n");
    summary.push_str("| Stage | Status | Duration | Commits |\n");
    summary.push_str("|-------|--------|----------|---------|\n");
    
    let mut total_commits = 0u32;
    let mut completed_count = 0;
    let mut failed_count = 0;
    let mut skipped_count = 0;
    
    for (i, stage_def) in PIPELINE_STAGES.iter().enumerate() {
        let stage_state = &state.stages[i];
        
        let (status_str, duration_str, commits_str) = match &stage_state.status {
            StageStatus::Pending => ("⏳ Pending".to_string(), "-".to_string(), "-".to_string()),
            StageStatus::Running => ("🔄 Running".to_string(), "-".to_string(), "-".to_string()),
            StageStatus::Complete { duration_secs, commits_processed } => {
                completed_count += 1;
                total_commits += commits_processed;
                (
                    "✅ Complete".to_string(),
                    format_duration(*duration_secs),
                    commits_processed.to_string(),
                )
            }
            StageStatus::Failed { error: _, attempts } => {
                failed_count += 1;
                (
                    format!("❌ Failed ({}x)", attempts),
                    "-".to_string(),
                    "-".to_string(),
                )
            }
            StageStatus::Skipped { reason: _ } => {
                skipped_count += 1;
                (
                    format!("⊘ Skipped"),
                    "-".to_string(),
                    "-".to_string(),
                )
            }
        };
        
        summary.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            stage_def.name, status_str, duration_str, commits_str
        ));
    }
    
    summary.push_str("\n### Summary\n\n");
    summary.push_str(&format!("- **Completed:** {} stages\n", completed_count));
    summary.push_str(&format!("- **Failed:** {} stages\n", failed_count));
    summary.push_str(&format!("- **Skipped:** {} stages\n", skipped_count));
    summary.push_str(&format!("- **Total Commits Processed:** {}\n", total_commits));
    
    summary
}

/// Format seconds as human-readable duration
fn format_duration(secs: u64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_pipeline_stages_order() {
        let names: Vec<_> = PIPELINE_STAGES.iter().map(|s| s.name).collect();
        assert_eq!(
            names,
            vec!["euler", "breaker", "hopper", "fowler", "carmack", "lamport", "huffman"]
        );
    }

    #[test]
    fn test_pipeline_state_new() {
        let state = PipelineState::new(10);
        assert_eq!(state.stages.len(), 7);
        assert_eq!(state.current_stage, 0);
        assert_eq!(state.commits_per_run, 10);
        assert!(state.stages.iter().all(|s| s.status == StageStatus::Pending));
    }

    #[test]
    fn test_pipeline_state_save_load() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();

        let state = PipelineState::new(10);
        state.save(repo_root).unwrap();

        let loaded = PipelineState::load(repo_root).unwrap().unwrap();
        assert_eq!(loaded.run_id, state.run_id);
        assert_eq!(loaded.stages.len(), 7);
    }

    #[test]
    fn test_pipeline_state_missing_returns_none() {
        let temp_dir = TempDir::new().unwrap();
        let result = PipelineState::load(temp_dir.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_pipeline_state_corrupted_returns_none() {
        let temp_dir = TempDir::new().unwrap();
        let repo_root = temp_dir.path();
        
        // Create corrupted state file
        let dir = PipelineState::sdlc_dir(repo_root);
        fs::create_dir_all(&dir).unwrap();
        fs::write(PipelineState::state_path(repo_root), "not valid json").unwrap();

        let result = PipelineState::load(repo_root).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_mark_complete_advances_stage() {
        let mut state = PipelineState::new(10);
        assert_eq!(state.current_stage, 0);
        
        state.mark_running();
        state.mark_complete(60, 5, "abc123");
        
        assert_eq!(state.current_stage, 1);
        assert!(matches!(state.stages[0].status, StageStatus::Complete { .. }));
        assert_eq!(state.commit_cursor, Some("abc123".to_string()));
    }

    #[test]
    fn test_mark_failed_tracks_attempts() {
        let mut state = PipelineState::new(10);
        
        state.mark_failed("error 1");
        if let StageStatus::Failed { attempts, .. } = &state.stages[0].status {
            assert_eq!(*attempts, 1);
        } else {
            panic!("Expected Failed status");
        }

        state.mark_failed("error 2");
        if let StageStatus::Failed { attempts, .. } = &state.stages[0].status {
            assert_eq!(*attempts, 2);
        } else {
            panic!("Expected Failed status");
        }
    }

    #[test]
    fn test_retry_stage() {
        let mut state = PipelineState::new(10);
        state.mark_failed("some error");
        
        state.retry_stage().unwrap();
        assert_eq!(state.stages[0].status, StageStatus::Pending);
    }

    #[test]
    fn test_retry_non_failed_stage_errors() {
        let mut state = PipelineState::new(10);
        let result = state.retry_stage();
        assert!(result.is_err());
    }

    #[test]
    fn test_find_resume_point() {
        let mut state = PipelineState::new(10);
        
        // Complete first two stages
        state.mark_running();
        state.mark_complete(60, 5, "abc");
        state.mark_running();
        state.mark_complete(60, 5, "def");
        
        // Fail the third
        state.mark_failed("error");
        
        assert_eq!(state.find_resume_point(), 2);
    }

    #[test]
    fn test_resume_from_running_state() {
        let mut state = PipelineState::new(10);
        state.mark_running();
        
        // Simulate crash - stage is still "running"
        state.resume();
        
        assert_eq!(state.current_stage, 0);
        assert_eq!(state.stages[0].status, StageStatus::Pending);
    }

    #[test]
    fn test_is_complete() {
        let mut state = PipelineState::new(10);
        assert!(!state.is_complete());
        
        // Complete all stages
        for _ in 0..7 {
            state.mark_running();
            state.mark_complete(60, 5, "abc");
        }
        
        assert!(state.is_complete());
    }

    #[test]
    fn test_get_stage() {
        assert!(get_stage("euler").is_some());
        assert!(get_stage("unknown").is_none());
    }

    #[test]
    fn test_get_stage_index() {
        assert_eq!(get_stage_index("euler"), Some(0));
        assert_eq!(get_stage_index("breaker"), Some(1));
        assert_eq!(get_stage_index("huffman"), Some(6));
        assert_eq!(get_stage_index("unknown"), None);
    }
}
