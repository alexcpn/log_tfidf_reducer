export interface Stats {
  input_lines: number;
  kept_lines: number;
  input_tokens: number;
  output_tokens: number;
}

export interface ReduceLogOutput {
  text: string;
  stats: Stats;
}

export interface LogDetectionResult {
  detected: boolean;
  type: "path" | "inline" | null;
  content: string | null;
  startIndex: number | null;
  endIndex: number | null;
}
