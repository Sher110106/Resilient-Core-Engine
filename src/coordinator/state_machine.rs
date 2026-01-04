use crate::coordinator::error::{CoordinatorError, CoordinatorResult};
use crate::coordinator::types::{TransferEvent, TransferState};
use parking_lot::RwLock;
use std::sync::Arc;
use tokio::sync::mpsc;

pub struct TransferStateMachine {
    state: Arc<RwLock<TransferState>>,
    event_tx: mpsc::UnboundedSender<TransferEvent>,
    event_rx: Arc<RwLock<Option<mpsc::UnboundedReceiver<TransferEvent>>>>,
}

impl Default for TransferStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl TransferStateMachine {
    pub fn new() -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        Self {
            state: Arc::new(RwLock::new(TransferState::Idle)),
            event_tx,
            event_rx: Arc::new(RwLock::new(Some(event_rx))),
        }
    }

    /// Get current state
    pub fn current_state(&self) -> TransferState {
        self.state.read().clone()
    }

    /// Send event to state machine
    pub fn send_event(&self, event: TransferEvent) -> CoordinatorResult<()> {
        self.event_tx
            .send(event)
            .map_err(|e| CoordinatorError::InvalidStateTransition(e.to_string()))
    }

    /// Transition state based on event
    pub fn transition(&self, event: TransferEvent) -> CoordinatorResult<TransferState> {
        let mut state = self.state.write();

        let new_state = match (&*state, &event) {
            // Starting transfer
            (TransferState::Idle, TransferEvent::Start { .. }) => TransferState::Preparing,

            // First chunk completed, start transferring
            (TransferState::Preparing, TransferEvent::ChunkCompleted { .. }) => {
                TransferState::Transferring { progress: 0.0 }
            }

            // Chunk completed during transfer
            (TransferState::Transferring { progress }, TransferEvent::ChunkCompleted { .. }) => {
                TransferState::Transferring {
                    progress: *progress,
                }
            }

            // Chunk failed during transfer
            (TransferState::Transferring { progress }, TransferEvent::ChunkFailed { .. }) => {
                TransferState::Transferring {
                    progress: *progress,
                }
            }

            // Pause transfer
            (TransferState::Transferring { .. }, TransferEvent::Pause) => TransferState::Paused {
                reason: "User requested".into(),
            },

            // Resume transfer
            (TransferState::Paused { .. }, TransferEvent::Resume) => {
                TransferState::Transferring { progress: 0.0 }
            }

            // Network failure
            (
                TransferState::Transferring { progress: _ },
                TransferEvent::NetworkFailure { path_id },
            ) => TransferState::Paused {
                reason: format!("Network failure on path: {path_id}"),
            },

            // Network recovered
            (TransferState::Paused { reason }, TransferEvent::NetworkRecovered { .. })
                if reason.contains("Network failure") =>
            {
                TransferState::Transferring { progress: 0.0 }
            }

            // Complete transfer
            (TransferState::Transferring { .. }, TransferEvent::TransferComplete) => {
                TransferState::Completing
            }

            (TransferState::Completing, _) => TransferState::Completed,

            // Cancel transfer
            (_, TransferEvent::Cancel) => TransferState::Failed {
                error: "Cancelled by user".into(),
            },

            // Invalid transition
            _ => {
                return Err(CoordinatorError::InvalidStateTransition(format!(
                    "Cannot handle {:?} in state {:?}",
                    event, *state
                )));
            }
        };

        *state = new_state.clone();
        Ok(new_state)
    }

    /// Take event receiver (can only be called once)
    pub fn take_receiver(&self) -> Option<mpsc::UnboundedReceiver<TransferEvent>> {
        self.event_rx.write().take()
    }
}

impl Clone for TransferStateMachine {
    fn clone(&self) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        Self {
            state: self.state.clone(),
            event_tx,
            event_rx: Arc::new(RwLock::new(Some(event_rx))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::Priority;
    use std::path::PathBuf;

    #[test]
    fn test_state_machine_creation() {
        let sm = TransferStateMachine::new();
        assert_eq!(sm.current_state(), TransferState::Idle);
    }

    #[test]
    fn test_start_transition() {
        let sm = TransferStateMachine::new();
        let result = sm.transition(TransferEvent::Start {
            file_path: PathBuf::from("test.bin"),
            priority: Priority::Normal,
        });

        assert!(result.is_ok());
        assert_eq!(sm.current_state(), TransferState::Preparing);
    }

    #[test]
    fn test_preparing_to_transferring() {
        let sm = TransferStateMachine::new();

        sm.transition(TransferEvent::Start {
            file_path: PathBuf::from("test.bin"),
            priority: Priority::Normal,
        })
        .unwrap();

        sm.transition(TransferEvent::ChunkCompleted { chunk_number: 0 })
            .unwrap();

        match sm.current_state() {
            TransferState::Transferring { .. } => {}
            _ => panic!("Expected Transferring state"),
        }
    }

    #[test]
    fn test_pause_resume() {
        let sm = TransferStateMachine::new();

        sm.transition(TransferEvent::Start {
            file_path: PathBuf::from("test.bin"),
            priority: Priority::Normal,
        })
        .unwrap();

        sm.transition(TransferEvent::ChunkCompleted { chunk_number: 0 })
            .unwrap();
        sm.transition(TransferEvent::Pause).unwrap();

        assert!(sm.current_state().is_paused());

        sm.transition(TransferEvent::Resume).unwrap();
        assert!(sm.current_state().is_active());
    }

    #[test]
    fn test_cancel() {
        let sm = TransferStateMachine::new();

        sm.transition(TransferEvent::Start {
            file_path: PathBuf::from("test.bin"),
            priority: Priority::Normal,
        })
        .unwrap();

        sm.transition(TransferEvent::Cancel).unwrap();

        match sm.current_state() {
            TransferState::Failed { error } => {
                assert!(error.contains("Cancelled"));
            }
            _ => panic!("Expected Failed state"),
        }
    }

    #[test]
    fn test_invalid_transition() {
        let sm = TransferStateMachine::new();

        // Cannot pause from Idle
        let result = sm.transition(TransferEvent::Pause);
        assert!(result.is_err());
    }

    #[test]
    fn test_network_failure_recovery() {
        let sm = TransferStateMachine::new();

        sm.transition(TransferEvent::Start {
            file_path: PathBuf::from("test.bin"),
            priority: Priority::Normal,
        })
        .unwrap();

        sm.transition(TransferEvent::ChunkCompleted { chunk_number: 0 })
            .unwrap();
        sm.transition(TransferEvent::NetworkFailure {
            path_id: "path-0".into(),
        })
        .unwrap();

        assert!(sm.current_state().is_paused());

        sm.transition(TransferEvent::NetworkRecovered {
            path_id: "path-0".into(),
        })
        .unwrap();
        assert!(sm.current_state().is_active());
    }
}
