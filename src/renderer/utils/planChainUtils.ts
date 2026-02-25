import type { Session } from '@renderer/types/data';

export interface PlanChainLink {
  /** The session that preceded this one in the plan chain (the "plan" session) */
  prevSessionId?: string;
  /** The session that follows this one in the plan chain (the "implementation" session) */
  nextSessionId?: string;
}

/**
 * Build a map from session ID to plan chain links.
 *
 * Sessions sharing the same slug form a chronological chain.
 * Each session with hasPlanContent was continued from the previous session
 * in the chain (its "plan" session). Each session that precedes a
 * hasPlanContent session is the "plan" for that implementation.
 *
 * Supports chains of any length (plan → impl → impl → ...).
 */
export function buildPlanChainMap(sessions: Session[]): Map<string, PlanChainLink> {
  // Group sessions by slug
  const bySlug = new Map<string, Session[]>();
  for (const session of sessions) {
    if (!session.slug) continue;
    let group = bySlug.get(session.slug);
    if (!group) {
      group = [];
      bySlug.set(session.slug, group);
    }
    group.push(session);
  }

  const result = new Map<string, PlanChainLink>();

  for (const group of bySlug.values()) {
    if (group.length < 2) continue;

    // Sort by creation time ascending (oldest first)
    group.sort((a, b) => a.createdAt - b.createdAt);

    // Link consecutive sessions where the later one has hasPlanContent
    for (let i = 1; i < group.length; i++) {
      if (group[i].hasPlanContent) {
        const prevId = group[i - 1].id;
        const currId = group[i].id;

        // Current session points back to its plan
        const currLink = result.get(currId) ?? {};
        currLink.prevSessionId = prevId;
        result.set(currId, currLink);

        // Previous session points forward to its implementation
        const prevLink = result.get(prevId) ?? {};
        prevLink.nextSessionId = currId;
        result.set(prevId, prevLink);
      }
    }
  }

  return result;
}
