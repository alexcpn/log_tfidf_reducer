# Feature Specification: TF-IDF Log Reduction for LLM Token Savings

**Feature Branch**: `001-tfidf-log-reduction`

**Created**: 2026-06-01

**Status**: Draft

**Input**: User description: "Reduce noisy log files via TF-IDF ranking so only the most informative lines are sent to an LLM, cutting token usage and cost"

## Clarifications

### Session 2026-06-01

- Q: Should the tool redact sensitive data before output is sent to an external LLM? → A: Yes — redact a built-in set of secrets/PII by default (best-effort, via known-shape patterns plus a high-entropy backstop)
- Q: When a high-priority line is kept, should surrounding lines be kept for context? → A: Yes — keep a small configurable window around each kept line (window of 0 disables it)
- Q: How is a line's severity determined when there is no explicit level field? → A: Use the explicit level field when present; otherwise infer from common message keywords/patterns (error, fatal, exception, panic, traceback)

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Shrink a log to a token budget without losing the incident (Priority: P1)

An engineer investigating an incident has a large, noisy log file (thousands to
millions of lines) and wants to ask an LLM about it, but the raw log is far too
large and expensive to send. They run the tool on the log file and get back a
much smaller file that keeps the unusual and severe lines (errors, rare events,
state changes) while suppressing repetitive boilerplate — small enough to fit in
the LLM's context and their budget.

**Why this priority**: This is the entire point of the feature. Without it there
is no product. It delivers value on its own: a smaller, faithful log file the
engineer can paste into any LLM.

**Independent Test**: Run the tool on a sample noisy log with a known
problem buried in it, at a chosen size/token budget, and confirm the reduced
output is dramatically smaller yet still contains the error lines and at least
one example of every distinct kind of line.

**Acceptance Scenarios**:

1. **Given** a 1,000,000-line log that is mostly repeated health-check lines with
   a handful of errors, **When** the engineer reduces it to an 8,000-token
   budget, **Then** the output is within that budget and contains every error
   line plus a representative of each distinct message type.
2. **Given** a log containing a crash-looping error repeated 50,000 times,
   **When** it is reduced, **Then** the output still surfaces that error (it is
   not discarded for being frequent) and shows how many times it occurred.
3. **Given** any input log, **When** reduction runs, **Then** no distinct kind of
   message is dropped entirely without at least one representative line
   appearing in the output.

---

### User Story 2 - Readable, summarized output an LLM can reason over (Priority: P2)

The engineer (and the LLM) needs the reduced log to read like a coherent
timeline, not a jumbled ranked list. The output preserves chronological order,
collapses repeated lines into a single representative annotated with a count,
marks where lines were omitted, and starts with a short summary of the log's
shape (time span, counts by severity, the most common message types).

**Why this priority**: Reduction that produces an unreadable or misleading
ordering undermines the analysis. This makes the P1 output trustworthy and
useful, but P1 already delivers a usable artifact.

**Independent Test**: Reduce a log and confirm the output is in time order,
repeated lines show counts (e.g. "×412"), omitted spans are marked, and a header
summary is present and accurate.

**Acceptance Scenarios**:

1. **Given** a reduced log, **When** the engineer opens it, **Then** lines appear
   in chronological order with a header summarizing total lines, time span, and
   severity counts.
2. **Given** a message type that occurred many times, **When** it is collapsed,
   **Then** the output shows one representative line with an accurate occurrence
   count rather than repeating it.
3. **Given** a region of the log that was omitted, **When** the engineer reads
   the output, **Then** a marker indicates how many lines were omitted and their
   general nature.

---

### User Story 3 - One-step analysis with an LLM (Priority: P3)

After producing a reduced log, the engineer optionally hands it to an LLM to get
a plain-language summary or root-cause hypothesis, without manually copying and
pasting. The analysis step consumes the reduced file and is fully optional and
separate from reduction.

**Why this priority**: Convenience layer. The reduction output is already
valuable on its own; this just closes the loop. It depends on external service
access and credentials, so it must not block the core tool.

**Independent Test**: Take an already-reduced file and run the optional analysis
step; confirm it returns an LLM response and that reduction works identically
whether or not this step is ever used.

**Acceptance Scenarios**:

1. **Given** a reduced log file, **When** the engineer runs the optional analysis
   with valid credentials, **Then** they receive an LLM-generated summary of the
   log.
2. **Given** no credentials or no network, **When** the engineer runs only
   reduction, **Then** reduction completes successfully and produces its output
   unaffected.

---

### Edge Cases

- **Empty or whitespace-only log**: produces an empty (or header-only) output and
  exits successfully, not an error.
- **Log with no timestamps**: still reduces; chronological ordering falls back to
  original input order, and the header omits the time span.
- **Every line identical**: collapses to a single representative line with the
  total count.
- **Every line unique**: nothing can be collapsed; the tool still honors the
  token budget by keeping the highest-ranked lines and marking the rest omitted.
- **Multi-line entries (e.g. stack traces)**: a multi-line entry is treated as
  one logical record, not split into unrelated noisy lines.
- **Budget smaller than a single line**: emits at least the single
  highest-priority record (or a clear notice) rather than empty/garbage output.
- **Malformed, binary, or extremely long lines**: handled without crashing;
  unparseable lines are treated as plain messages.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST accept a log file as input and produce a reduced
  output file (or stream) intended for submission to an LLM.
- **FR-002**: System MUST rank log lines by how informative/unusual they are, so
  rare and meaningful lines are favored over high-frequency boilerplate.
- **FR-003**: System MUST normalize per-line variable data (timestamps,
  identifiers, addresses, numbers) before judging similarity, so that lines
  differing only in such values are recognized as the same kind of message.
- **FR-004**: System MUST guarantee that at least one representative line is kept
  for every distinct kind of message before any kind contributes additional
  lines (no kind is silently dropped entirely).
- **FR-005**: System MUST preserve high-severity lines (errors and fatals) even
  when they are highly repetitive, surfacing them with an occurrence count.
- **FR-005a**: System MUST determine a line's severity from an explicit level
  field when one is present, and otherwise MUST infer it from common message
  keywords/patterns (e.g. error, fatal, exception, panic, traceback), so the
  FR-005 guarantee holds for logs without a structured level field.
- **FR-006**: System MUST let the user specify a target size as a token budget
  and MUST keep the output at or under that budget.
- **FR-007**: System MUST collapse repeated occurrences of the same kind of
  message into a single representative annotated with an accurate count.
- **FR-008**: System MUST present the kept lines in chronological order (falling
  back to input order when timestamps are absent) and MUST mark omitted spans.
- **FR-009**: System MUST prepend a concise summary of the log (e.g. total line
  count, time span, counts by severity, most common message types).
- **FR-010**: System MUST produce identical output for identical input and
  configuration (deterministic, stable ordering and tie-breaking).
- **FR-011**: System MUST report how much reduction was achieved (input vs.
  output size / token counts).
- **FR-012**: System MUST perform reduction without requiring any network access
  or LLM service; the optional LLM analysis step MUST be separate and skippable.
- **FR-013**: System MUST handle the edge cases listed above without crashing.
- **FR-014**: System MUST redact sensitive data (secrets/credentials such as API
  keys, tokens, passwords, and PII such as email addresses) from the reduced
  output by default, so they are not transmitted to an external LLM. Redaction is
  best-effort (recognized patterns plus a high-entropy backstop); residual risk
  of missed values MUST be disclosed to the user.
- **FR-015**: When the system keeps a high-priority line, it MUST also keep a
  small surrounding window of adjacent lines for context. The window size MUST be
  user-configurable, and a size of zero MUST disable it (pure rank/budget
  selection). Kept context lines count against the token budget.

### Key Entities *(include if feature involves data)*

- **Log record**: a single logical log entry (one or more physical lines),
  carrying an optional timestamp, an optional severity level, and a message.
- **Message type (template)**: a group of log records that are the same once
  their variable data is normalized; carries an occurrence count and a
  representative example.
- **Reduced output**: the LLM-ready artifact — a header summary plus the selected
  records in chronological order with collapse counts and omission markers.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: On a representative noisy production log, the reduced output is no
  more than 10% of the original token count.
- **SC-002**: At least 95% of human-labeled "incident-relevant" lines (including
  all error/fatal lines) survive reduction at a moderate budget.
- **SC-003**: Every distinct message type present in the input appears at least
  once in the output (100%).
- **SC-004**: The tool reduces a 1 GB / multi-million-line log in under 60
  seconds on a typical developer laptop.
- **SC-005**: Re-running the tool on the same input and settings yields byte-for-
  byte identical output.
- **SC-006**: An engineer can go from a raw log file to an LLM-ready reduced file
  in a single command.
- **SC-007**: Across a test corpus seeded with known secret/PII values, 100% of
  those known-pattern values are redacted from the output (best-effort patterns
  plus high-entropy backstop; novel formats are out of scope of this guarantee).

## Assumptions

- Reduction operates on a whole file in one batch; live/streaming tailing is out
  of scope for the first version.
- Input is text-based log data in common formats (plain lines, key-value,
  JSON-per-line, syslog/kernel-style); structured tracing systems are out of
  scope.
- The token budget is a user-configurable input; an approximate token count is
  acceptable for budgeting purposes and any approximation is disclosed to the
  user.
- The optional LLM analysis step relies on an external LLM service and the user
  supplying their own credentials; it is decoupled from and never required by
  reduction.
- "Informativeness" ranking is combined with severity and frequency handling so
  that rare-but-trivial noise is not over-favored and frequent-but-critical
  errors are not lost (this trade-off is governed by the project constitution).
