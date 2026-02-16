/**
 * useTokenCalculations - Encapsulates token calculation logic for TokenUsageDisplay.
 */

import { formatTokensCompact as formatTokens } from '@shared/utils/tokenFormatting';

interface TokenCalculationsResult {
  totalTokens: number;
  formattedTotal: string;
}

export function useTokenCalculations(
  inputTokens: number,
  outputTokens: number,
  cacheReadTokens: number,
  cacheCreationTokens: number
): TokenCalculationsResult {
  const totalTokens = inputTokens + cacheReadTokens + cacheCreationTokens + outputTokens;
  const formattedTotal = formatTokens(totalTokens);

  return { totalTokens, formattedTotal };
}
