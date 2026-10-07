# ADR-0008: Semantic Command Transactions and Snapshot-Based Background Work

Status: **Accepted**  
Date: **2026-10-07**

## Context

The editor needs undo/redo, continuous terrain gestures, multi-region edits, autosave, validation, rendering, export, and asynchronous work without allowing panels or GPU resources to become authoritative state.

A global mutable application object shared behind long-lived locks would make command ordering, undo correctness, stale worker results, and recovery substantially harder to reason about.

## Decision

All persistent editor mutations execute through semantic commands grouped into user-intent transactions.

The editor will:

- serialize live semantic document mutations through one command dispatcher;
- require every committed transaction to be exactly reversible or to retain sufficient before-state for exact reversal;
- represent derived invalidation as typed `ChangeSet` data rather than direct GPU mutations;
- use immutable generation-tagged snapshots for background work;
- reject/discard stale asynchronous results by generation comparison;
- keep preview/session state outside semantic history unless explicitly project-owned;
- coalesce compatible repetitive commands only across explicit user-intent boundaries.

Workers may perform scene materialization, render compilation, validation, autosave, thumbnails, and export from snapshots, but they do not hold long-lived mutable access to the live document.

## Consequences

### Positive

- deterministic undo/redo;
- tool cancellation can restore exact state;
- renderer/device lifecycle is decoupled from edit history;
- stale async completions cannot silently overwrite newer work;
- cross-region edits remain one coherent user transaction;
- command tests can prove exact apply/revert behavior.

### Costs

- commands need explicit inverse/before-state design;
- large terrain strokes require compact delta storage;
- background jobs need generation plumbing;
- derived systems must consume change/invalidation events rather than reaching into mutable UI state.

## Alternatives considered

### Snapshot the entire project for every undo step

Rejected as the default because it scales poorly for large multi-region projects, though snapshots may still be used for checkpoints/autosave.

### Direct model mutation from widgets/tools

Rejected because it creates multiple mutation paths and weakens undo, validation, and testing.

### One global `Arc<RwLock<AppState>>`

Rejected as the primary coordination model because it encourages broad coupling and long-lived locks across UI/render/background tasks.

## Constraints / invariants affected

- D3: scene build is deterministic.
- D4: dirty tracking starts from semantic edits.
- F1: editor history uses commands/transactions.
- H1/H2: errors/spec behavior remain diagnosable.

## Follow-up work

- define concrete command/transaction traits during implementation;
- choose compact terrain-delta representation;
- define typed application event/change bus;
- test randomized apply/undo sequences;
- benchmark large brush transactions.

## Supersedes / superseded by

None.
