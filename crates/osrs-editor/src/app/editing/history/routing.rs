//! Chronological undo and redo routing across semantic and legacy history entries.

use super::*;

impl EditorApp {
    pub(in crate::app) fn undo(&mut self) {
        let Some(step) = self.history.pop() else {
            return;
        };
        if let Some(paint) = step.semantic_paint.clone() {
            match self.session.undo() {
                Ok(Some(_)) => match self.project_semantic_paint_history(&paint) {
                    Ok(count) => {
                        self.notice = Some((
                            format!(
                                "Undid {count} painted tile{}",
                                if count == 1 { "" } else { "s" }
                            ),
                            Instant::now(),
                        ));
                        self.redo_stack.push(step);
                    }
                    Err(message) => {
                        let rollback = self.session.redo();
                        self.history.push(step);
                        self.notice = Some((
                            if rollback.is_ok() {
                                format!("Paint undo was rolled back: {message}")
                            } else {
                                format!(
                                    "Paint undo projection failed and rollback failed: {message}"
                                )
                            },
                            Instant::now(),
                        ));
                    }
                },
                Ok(None) => {
                    self.history.push(step);
                    self.notice = Some((
                        "Paint undo history is out of sync with the semantic session".to_owned(),
                        Instant::now(),
                    ));
                }
                Err(error) => {
                    self.history.push(step);
                    self.notice = Some((
                        format!("Paint undo was rejected by the semantic session: {error:?}"),
                        Instant::now(),
                    ));
                }
            }
            return;
        }
        if let Some(height) = step.semantic_height.clone() {
            match self.session.undo() {
                Ok(Some(_)) => match self.project_semantic_height_history(&height) {
                    Ok(count) => {
                        self.notice = Some((
                            format!(
                                "Undid {count} Height tile{}",
                                if count == 1 { "" } else { "s" }
                            ),
                            Instant::now(),
                        ));
                        self.redo_stack.push(step);
                    }
                    Err(message) => {
                        let rollback_succeeded = matches!(self.session.redo(), Ok(Some(_)));
                        self.history.push(step);
                        self.notice = Some((
                            if rollback_succeeded {
                                format!("Height undo was rolled back: {message}")
                            } else {
                                format!(
                                    "Height undo projection failed and rollback failed: {message}"
                                )
                            },
                            Instant::now(),
                        ));
                    }
                },
                Ok(None) => {
                    self.history.push(step);
                    self.notice = Some((
                        "Height undo history is out of sync with the semantic session".to_owned(),
                        Instant::now(),
                    ));
                }
                Err(error) => {
                    self.history.push(step);
                    self.notice = Some((
                        format!("Height undo was rejected by the semantic session: {error:?}"),
                        Instant::now(),
                    ));
                }
            }
            return;
        }
        if let Some(path) = step.semantic_path.clone() {
            match self.session.undo() {
                Ok(Some(_)) => match self.project_semantic_path_history(&path) {
                    Ok(count) => {
                        self.notice = Some((
                            format!(
                                "Undid {count} path tile{}",
                                if count == 1 { "" } else { "s" }
                            ),
                            Instant::now(),
                        ));
                        self.redo_stack.push(step);
                    }
                    Err(message) => {
                        let rollback_succeeded = matches!(self.session.redo(), Ok(Some(_)));
                        self.history.push(step);
                        self.notice = Some((
                            if rollback_succeeded {
                                format!("Path undo was rolled back: {message}")
                            } else {
                                format!(
                                    "Path undo projection failed and rollback failed: {message}"
                                )
                            },
                            Instant::now(),
                        ));
                    }
                },
                Ok(None) => {
                    self.history.push(step);
                    self.notice = Some((
                        "Path undo history is out of sync with the semantic session".to_owned(),
                        Instant::now(),
                    ));
                }
                Err(error) => {
                    self.history.push(step);
                    self.notice = Some((
                        format!("Path undo was rejected by the semantic session: {error:?}"),
                        Instant::now(),
                    ));
                }
            }
            return;
        }
        if let Some(object) = step.semantic_object.clone() {
            match self.session.undo() {
                Ok(Some(_)) => match self.project_semantic_object_history(&object, false) {
                    Ok(count) => {
                        self.notice = Some((
                            format!("Undid {count} object{}", if count == 1 { "" } else { "s" }),
                            Instant::now(),
                        ));
                        self.redo_stack.push(step);
                    }
                    Err(message) => {
                        let rollback_succeeded = matches!(self.session.redo(), Ok(Some(_)));
                        self.history.push(step);
                        self.notice = Some((
                            if rollback_succeeded {
                                format!("Object undo was rolled back: {message}")
                            } else {
                                format!(
                                    "Object undo projection failed and rollback failed: {message}"
                                )
                            },
                            Instant::now(),
                        ));
                    }
                },
                Ok(None) => {
                    self.history.push(step);
                    self.notice = Some((
                        "Object undo history is out of sync with the semantic session".to_owned(),
                        Instant::now(),
                    ));
                }
                Err(error) => {
                    self.history.push(step);
                    self.notice = Some((
                        format!("Object undo was rejected by the semantic session: {error:?}"),
                        Instant::now(),
                    ));
                }
            }
            return;
        }

        self.revert(&step, false);
        self.notice = Some((
            format!("Undid {} tile change(s)", step.changes.len()),
            Instant::now(),
        ));
        self.redo_stack.push(step);
    }

    pub(in crate::app) fn redo(&mut self) {
        let Some(step) = self.redo_stack.pop() else {
            return;
        };
        if let Some(paint) = step.semantic_paint.clone() {
            match self.session.redo() {
                Ok(Some(_)) => match self.project_semantic_paint_history(&paint) {
                    Ok(count) => {
                        self.notice = Some((
                            format!(
                                "Redid {count} painted tile{}",
                                if count == 1 { "" } else { "s" }
                            ),
                            Instant::now(),
                        ));
                        self.history.push(step);
                    }
                    Err(message) => {
                        let rollback = self.session.undo();
                        self.redo_stack.push(step);
                        self.notice = Some((
                            if rollback.is_ok() {
                                format!("Paint redo was rolled back: {message}")
                            } else {
                                format!(
                                    "Paint redo projection failed and rollback failed: {message}"
                                )
                            },
                            Instant::now(),
                        ));
                    }
                },
                Ok(None) => {
                    self.redo_stack.push(step);
                    self.notice = Some((
                        "Paint redo history is out of sync with the semantic session".to_owned(),
                        Instant::now(),
                    ));
                }
                Err(error) => {
                    self.redo_stack.push(step);
                    self.notice = Some((
                        format!("Paint redo was rejected by the semantic session: {error:?}"),
                        Instant::now(),
                    ));
                }
            }
            return;
        }
        if let Some(height) = step.semantic_height.clone() {
            match self.session.redo() {
                Ok(Some(_)) => match self.project_semantic_height_history(&height) {
                    Ok(count) => {
                        self.notice = Some((
                            format!(
                                "Redid {count} Height tile{}",
                                if count == 1 { "" } else { "s" }
                            ),
                            Instant::now(),
                        ));
                        self.history.push(step);
                    }
                    Err(message) => {
                        let rollback_succeeded = matches!(self.session.undo(), Ok(Some(_)));
                        self.redo_stack.push(step);
                        self.notice = Some((
                            if rollback_succeeded {
                                format!("Height redo was rolled back: {message}")
                            } else {
                                format!(
                                    "Height redo projection failed and rollback failed: {message}"
                                )
                            },
                            Instant::now(),
                        ));
                    }
                },
                Ok(None) => {
                    self.redo_stack.push(step);
                    self.notice = Some((
                        "Height redo history is out of sync with the semantic session".to_owned(),
                        Instant::now(),
                    ));
                }
                Err(error) => {
                    self.redo_stack.push(step);
                    self.notice = Some((
                        format!("Height redo was rejected by the semantic session: {error:?}"),
                        Instant::now(),
                    ));
                }
            }
            return;
        }
        if let Some(path) = step.semantic_path.clone() {
            match self.session.redo() {
                Ok(Some(_)) => match self.project_semantic_path_history(&path) {
                    Ok(count) => {
                        self.notice = Some((
                            format!(
                                "Redid {count} path tile{}",
                                if count == 1 { "" } else { "s" }
                            ),
                            Instant::now(),
                        ));
                        self.history.push(step);
                    }
                    Err(message) => {
                        let rollback_succeeded = matches!(self.session.undo(), Ok(Some(_)));
                        self.redo_stack.push(step);
                        self.notice = Some((
                            if rollback_succeeded {
                                format!("Path redo was rolled back: {message}")
                            } else {
                                format!(
                                    "Path redo projection failed and rollback failed: {message}"
                                )
                            },
                            Instant::now(),
                        ));
                    }
                },
                Ok(None) => {
                    self.redo_stack.push(step);
                    self.notice = Some((
                        "Path redo history is out of sync with the semantic session".to_owned(),
                        Instant::now(),
                    ));
                }
                Err(error) => {
                    self.redo_stack.push(step);
                    self.notice = Some((
                        format!("Path redo was rejected by the semantic session: {error:?}"),
                        Instant::now(),
                    ));
                }
            }
            return;
        }
        if let Some(object) = step.semantic_object.clone() {
            match self.session.redo() {
                Ok(Some(_)) => match self.project_semantic_object_history(&object, true) {
                    Ok(count) => {
                        self.notice = Some((
                            format!("Redid {count} object{}", if count == 1 { "" } else { "s" }),
                            Instant::now(),
                        ));
                        self.history.push(step);
                    }
                    Err(message) => {
                        let rollback_succeeded = matches!(self.session.undo(), Ok(Some(_)));
                        self.redo_stack.push(step);
                        self.notice = Some((
                            if rollback_succeeded {
                                format!("Object redo was rolled back: {message}")
                            } else {
                                format!(
                                    "Object redo projection failed and rollback failed: {message}"
                                )
                            },
                            Instant::now(),
                        ));
                    }
                },
                Ok(None) => {
                    self.redo_stack.push(step);
                    self.notice = Some((
                        "Object redo history is out of sync with the semantic session".to_owned(),
                        Instant::now(),
                    ));
                }
                Err(error) => {
                    self.redo_stack.push(step);
                    self.notice = Some((
                        format!("Object redo was rejected by the semantic session: {error:?}"),
                        Instant::now(),
                    ));
                }
            }
            return;
        }

        self.revert(&step, true);
        self.history.push(step);
    }
}
