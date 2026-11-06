## ADDED Requirements

### Requirement: Windows terminal polling SHALL drain a bounded batch of ready non-events
The Windows console input boundary SHALL skip at most 64 ready records that do
not produce a public event in one input attempt. It SHALL avoid an idle delay
between those records, retain partial UTF-16 across attempts, and yield to the
outer cancellation/deadline checks when its batch is exhausted. Existing event
numbers and Terminal APIs SHALL remain unchanged.

#### Scenario: Ignored records precede a ready Unicode key
- **WHEN** a short ready queue contains focus, KEYUP and disabled mouse records
  followed by an emoji surrogate pair, Chinese and ASCII keys
- **THEN** a zero-time poll returns the first complete key without an artificial
  empty event for each preceding record
- **AND** later polls preserve every following key and independently owned text.

#### Scenario: Ignored input continues past a polling batch
- **WHEN** more than one batch of ignored records is queued before a real key
- **THEN** a zero-time poll yields before draining the whole ignored queue
- **AND** cancellation remains observable before the next batch
- **AND** subsequent bounded polls eventually deliver the real key.

#### Scenario: A surrogate pair is split across polls
- **WHEN** one poll sees only the high surrogate and a later poll receives its
  matching low surrogate
- **THEN** the first poll returns no event and the later poll returns one exact
  Unicode key without discarding the partial scalar.
