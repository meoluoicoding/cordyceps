//! Game-phase detection and per-move time budgeting.

use crate::board::Board;
use crate::types::{GamePhase, SideConfig};

/// Detect game phase based on live mushroom count:
/// live > 32 -> Opening, 20-32 -> Midgame, 13-19 -> Late, <= 12 -> Endgame.
pub fn detect_phase(board: &Board) -> GamePhase {
    let live = board.live_count;
    if live > 32 {
        GamePhase::Opening
    } else if live >= 20 {
        GamePhase::Midgame
    } else if live > 12 {
        GamePhase::Late
    } else {
        GamePhase::Endgame
    }
}

#[allow(dead_code)]
pub fn estimate_moves_left(live_count: i32) -> i32 {
    if live_count > 60 {
        22
    } else if live_count > 40 {
        17
    } else if live_count > 25 {
        12
    } else if live_count > 12 {
        8
    } else {
        5
    }
}

fn phase_pct(live_count: i32, _config: &SideConfig) -> f32 {
    // Phase-based % of remaining time per move (from log analysis of winning engines).
    let phase = if live_count > 32 {
        GamePhase::Opening
    } else if live_count >= 20 {
        GamePhase::Midgame
    } else if live_count > 12 {
        GamePhase::Late
    } else {
        GamePhase::Endgame
    };

    match phase {
        GamePhase::Opening => 6.0,
        GamePhase::Midgame => 10.0,
        GamePhase::Late => 12.0,
        GamePhase::Endgame => 18.0,
    }
}

pub struct TimeManager;

impl TimeManager {
    pub fn new() -> Self {
        TimeManager
    }

    fn margin_factor(margin: i32) -> f32 {
        if margin > 40 {
            0.6
        } else if margin > 20 {
            0.7
        } else if margin > 5 {
            0.85
        } else if margin > -5 {
            1.0
        } else if margin > -20 {
            1.2
        } else if margin > -40 {
            1.35
        } else {
            1.5
        }
    }

    /// Per-move budget = remaining_ms * phase_pct/100 * side_mult * margin_factor,
    /// clamped to sane bounds.
    pub fn get_budget(&self, live_count: i32, config: &SideConfig, remaining_ms: i32, margin: i32) -> i32 {
        // Emergency: <500ms remaining -> fixed tiny budget.
        if remaining_ms < 500 {
            return 15;
        }

        let pct = phase_pct(live_count, config);
        let mut budget_f = remaining_ms as f32 * (pct / 100.0);

        // Side multiplier (FIRST=1.0, SECOND=1.5).
        budget_f *= config.time_multiplier;

        // Margin factor (winning=save, losing=invest).
        budget_f *= Self::margin_factor(margin);

        if budget_f < 10.0 {
            budget_f = 10.0;
        }

        // Generous cap: winning bots in logs spend up to 33%+.
        let max_budget = if live_count <= 12 { 2500.0 } else { 2000.0 };
        if budget_f > max_budget {
            budget_f = max_budget;
        }

        // Hard limit: never use > 90% of remaining.
        let hard_limit = remaining_ms as f32 * 0.9;
        if budget_f > hard_limit {
            budget_f = hard_limit;
        }

        budget_f as i32
    }
}
