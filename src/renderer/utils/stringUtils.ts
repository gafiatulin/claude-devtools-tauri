/**
 * String utilities for display formatting.
 */

/**
 * Truncates a string in the middle to preserve both the beginning and end.
 * Useful for branch names where the unique identifier is often at the end.
 *
 * @example
 * truncateMiddle("feature/very-long-branch-name-with-ticket-12345", 25)
 * // Returns: "feature/ver...ticket-12345"
 *
 * @param text - The string to truncate
 * @param maxLen - Maximum length of the resulting string (default: 25)
 * @returns The truncated string with "..." in the middle, or original if short enough
 */
export function truncateMiddle(text: string, maxLen: number = 25): string {
  if (!text || text.length <= maxLen) return text;

  // Account for the 3-character ellipsis
  const availableChars = maxLen - 3;
  const startLen = Math.ceil(availableChars / 2);
  const endLen = Math.floor(availableChars / 2);

  const start = text.slice(0, startLen);
  const end = text.slice(-endLen);

  return `${start}...${end}`;
}
