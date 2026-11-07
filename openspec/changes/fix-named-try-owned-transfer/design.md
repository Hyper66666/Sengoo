# Ownership at the question-mark branch

Evaluate the operand once and consume an owning named place before branching.
On success its payload belongs to the extracted value; on failure its error
container or rebuilt error belongs to the caller. The source cannot retain a
cleanup obligation on either path. Existing unrelated locals still clean up in
reverse declaration order, with the existing conditional Drop machinery.

Move checking must reject reusing an owning operand after question-mark
extraction. Copy-only scalar wrappers keep their existing behavior.
Verify with observable native JSON handle liveness and independently with a
compiler regression; rerun exact CLI retry, cancel and tool continuation checks.
