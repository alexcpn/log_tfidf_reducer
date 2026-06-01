#!/usr/bin/env python3
"""
Optional LLM analysis layer — decoupled from the Rust reducer (FR-012, Principle V).

Reads a reduced log file produced by `logreduce` and sends it to Claude via the
Anthropic SDK, with the log body in a cache_control block for prompt caching.
The reducer never imports or requires this script.
"""

import argparse
import os
import sys


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Analyze a reduced log with Claude (requires ANTHROPIC_API_KEY)"
    )
    parser.add_argument("reduced_file", help="Path to a reduced log file from logreduce")
    parser.add_argument(
        "--model",
        default="claude-haiku-4-5-20251001",
        help="Claude model ID (default: claude-haiku-4-5-20251001)",
    )
    parser.add_argument(
        "--question",
        default="Summarize what happened in this log and identify the root cause of any errors.",
        help="Question to ask Claude about the log",
    )
    args = parser.parse_args()

    api_key = os.environ.get("ANTHROPIC_API_KEY")
    if not api_key:
        print("Error: ANTHROPIC_API_KEY environment variable is not set.", file=sys.stderr)
        sys.exit(1)

    try:
        with open(args.reduced_file) as f:
            log_content = f.read()
    except OSError as e:
        print(f"Error reading {args.reduced_file}: {e}", file=sys.stderr)
        sys.exit(2)

    try:
        import anthropic
    except ImportError:
        print("Error: anthropic package not installed. Run: pip install anthropic", file=sys.stderr)
        sys.exit(1)

    client = anthropic.Anthropic(api_key=api_key)

    try:
        response = client.messages.create(
            model=args.model,
            max_tokens=1024,
            messages=[
                {
                    "role": "user",
                    "content": [
                        # Log body in a cache_control block for prompt caching
                        {
                            "type": "text",
                            "text": f"Here is a reduced log file:\n\n{log_content}",
                            "cache_control": {"type": "ephemeral"},
                        },
                        {
                            "type": "text",
                            "text": args.question,
                        },
                    ],
                }
            ],
        )
    except anthropic.AuthenticationError:
        print("Error: Invalid ANTHROPIC_API_KEY.", file=sys.stderr)
        sys.exit(1)
    except anthropic.APIConnectionError as e:
        print(f"Error: Could not connect to Anthropic API: {e}", file=sys.stderr)
        sys.exit(1)
    except anthropic.APIError as e:
        print(f"Error: Anthropic API error: {e}", file=sys.stderr)
        sys.exit(1)

    print(response.content[0].text)


if __name__ == "__main__":
    main()
