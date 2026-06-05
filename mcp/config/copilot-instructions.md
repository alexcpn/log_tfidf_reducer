# Copilot Instructions — Log Analysis

When a user asks you to analyse a log file or shares log content (more than a few lines),
first call the `reduce_log` MCP tool to compress the log to an 8000-token budget.
Use the reduced output (`result.text`) for your analysis.
Report the reduction ratio from `result.stats` if it is significant (>50% reduction).
