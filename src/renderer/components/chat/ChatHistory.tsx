import React, { useCallback, useDeferredValue, useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import ReactMarkdown from 'react-markdown';

import { useAutoScrollBottom } from '@renderer/hooks/useAutoScrollBottom';
import { useTabNavigationController } from '@renderer/hooks/useTabNavigationController';
import { useTabUI } from '@renderer/hooks/useTabUI';
import { useVisibleAIGroup } from '@renderer/hooks/useVisibleAIGroup';
import { useStore } from '@renderer/store';
import { MAX_PANES } from '@renderer/types/panes';
import { buildPlanChainMap } from '@renderer/utils/planChainUtils';
import { FileCheck } from 'lucide-react';
import remarkGfm from 'remark-gfm';

import { CopyButton } from '../common/CopyButton';
import { ChatScrollContainer } from './ChatScrollContainer';
import { markdownComponents } from './markdownComponents';
import { SessionContextPanel } from './SessionContextPanel/index';
import { ChatHistoryEmptyState } from './ChatHistoryEmptyState';
import { ChatHistoryLoadingState } from './ChatHistoryLoadingState';
import { useVirtualChatList } from './useVirtualChatList';

import type { ContextInjection } from '@renderer/types/contextInjection';

// Module-level: survives component unmount/remount on tab switch
let pendingPlanChainScroll: 'top' | 'bottom' | null = null;

interface ChatHistoryProps {
  /** Tab ID for per-tab state isolation (scroll position, deep links) */
  tabId?: string;
}

export const ChatHistory = ({ tabId }: ChatHistoryProps) => {
  const VIRTUALIZATION_THRESHOLD = 80;
  const ESTIMATED_CHAT_ITEM_HEIGHT = 260;

  // Per-tab UI state (context panel, scroll position, expansion) from useTabUI
  const {
    isContextPanelVisible,
    setContextPanelVisible,
    savedScrollTop,
    saveScrollPosition,
    expandAIGroup,
    expandSubagentTrace,
    selectedContextPhase,
    setSelectedContextPhase,
  } = useTabUI();

  // Search state (only re-renders when search-specific values change)
  const searchQuery = useStore((s) => s.searchQuery);
  const currentSearchIndex = useStore((s) => s.currentSearchIndex);
  const searchMatches = useStore((s) => s.searchMatches);
  const setSearchQuery = useStore((s) => s.setSearchQuery);
  const syncSearchMatchesWithRendered = useStore((s) => s.syncSearchMatchesWithRendered);
  const selectSearchMatch = useStore((s) => s.selectSearchMatch);

  // Tab state (only re-renders when tab-specific values change)
  const openTabs = useStore((s) => s.openTabs);
  const activeTabId = useStore((s) => s.activeTabId);
  const consumeTabNavigation = useStore((s) => s.consumeTabNavigation);
  const setTabVisibleAIGroup = useStore((s) => s.setTabVisibleAIGroup);

  // Per-tab session data (each selector only triggers re-render when its specific value changes)
  const conversation = useStore((s) => {
    const td = tabId ? s.tabSessionData[tabId] : null;
    return td?.conversation ?? s.conversation;
  });
  const conversationLoading = useStore((s) => {
    const td = tabId ? s.tabSessionData[tabId] : null;
    return td?.conversationLoading ?? s.conversationLoading;
  });
  const sessionContextStats = useStore((s) => {
    const td = tabId ? s.tabSessionData[tabId] : null;
    return td?.sessionContextStats ?? s.sessionContextStats;
  });
  const sessionPhaseInfo = useStore((s) => {
    const td = tabId ? s.tabSessionData[tabId] : null;
    return td?.sessionPhaseInfo ?? s.sessionPhaseInfo;
  });
  const sessionDetail = useStore((s) => {
    const td = tabId ? s.tabSessionData[tabId] : null;
    return td?.sessionDetail ?? s.sessionDetail;
  });

  // Sessions and navigation for plan chain banners
  const sessions = useStore((s) => s.sessions);
  const openTab = useStore((s) => s.openTab);
  const selectSession = useStore((s) => s.selectSession);
  const activeProjectId = useStore((s) => s.activeProjectId);
  const splitPane = useStore((s) => s.splitPane);
  const paneCount = useStore((s) => s.paneLayout.panes.length);

  // State for Context button hover (local state OK - doesn't need per-tab isolation)
  const [isContextButtonHovered, setIsContextButtonHovered] = useState(false);

  // Determine if this tab instance is currently active
  // Use tabId prop if provided, otherwise fall back to activeTabId (for backwards compatibility)
  const effectiveTabId = tabId ?? activeTabId;
  const isThisTabActive = effectiveTabId === activeTabId;

  // Get THIS tab's pending navigation request
  const thisTab = effectiveTabId ? openTabs.find((t) => t.id === effectiveTabId) : null;
  const pendingNavigation = thisTab?.pendingNavigation;

  // Defer context stats computation to avoid blocking rendering during updates
  const deferredSessionContextStats = useDeferredValue(sessionContextStats);
  const deferredConversation = useDeferredValue(conversation);
  const isContextStale = deferredSessionContextStats !== sessionContextStats || deferredConversation !== conversation;

  // Compute all accumulated context injections (phase-aware)
  const { allContextInjections, lastAiGroupTotalTokens } = useMemo(() => {
    if (!deferredSessionContextStats || !deferredConversation?.items.length) {
      return { allContextInjections: [] as ContextInjection[], lastAiGroupTotalTokens: undefined };
    }

    // Determine which phase to show
    const effectivePhase = selectedContextPhase;

    // If a specific phase is selected, find the last AI group in that phase
    let targetAiGroupId: string | undefined;
    if (effectivePhase !== null && sessionPhaseInfo) {
      const phase = sessionPhaseInfo.phases.find((p) => p.phaseNumber === effectivePhase);
      if (phase) {
        targetAiGroupId = phase.lastAIGroupId;
      }
    }

    // Default: use the last AI group overall
    if (!targetAiGroupId) {
      const lastAiItem = [...deferredConversation.items].reverse().find((item) => item.type === 'ai');
      if (lastAiItem?.type !== 'ai') {
        return {
          allContextInjections: [] as ContextInjection[],
          lastAiGroupTotalTokens: undefined,
        };
      }
      targetAiGroupId = lastAiItem.group.id;
    }

    const stats = deferredSessionContextStats.get(targetAiGroupId);
    const injections = stats?.accumulatedInjections ?? [];

    // Get total tokens from the target AI group
    let totalTokens: number | undefined;
    const targetItem = deferredConversation.items.find(
      (item) => item.type === 'ai' && item.group.id === targetAiGroupId
    );
    if (targetItem?.type === 'ai') {
      const responses = targetItem.group.responses || [];
      for (let i = responses.length - 1; i >= 0; i--) {
        const msg = responses[i];
        if (msg.type === 'assistant' && msg.usage) {
          const usage = msg.usage;
          totalTokens =
            (usage.input_tokens ?? 0) +
            (usage.output_tokens ?? 0) +
            (usage.cache_read_input_tokens ?? 0) +
            (usage.cache_creation_input_tokens ?? 0);
          break;
        }
      }
    }

    return { allContextInjections: injections, lastAiGroupTotalTokens: totalTokens };
  }, [deferredSessionContextStats, deferredConversation, selectedContextPhase, sessionPhaseInfo]);

  // State for navigation highlight (blue, used for Turn navigation from CLAUDE.md panel)
  const [isNavigationHighlight, setIsNavigationHighlight] = useState(false);
  const navigationHighlightTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Refs map for AI groups, chat items, and individual tool items (for scrolling)
  const aiGroupRefs = useRef<Map<string, HTMLElement>>(new Map());
  const chatItemRefs = useRef<Map<string, HTMLElement>>(new Map());
  const toolItemRefs = useRef<Map<string, HTMLElement>>(new Map());

  // Shared scroll container ref - used by both auto-scroll and navigation coordinator
  const scrollContainerRef = useRef<HTMLDivElement>(null);

  const isSearchActive = searchQuery.trim().length > 0;
  const emptyRenderedSyncCountRef = useRef(0);

  // Virtual list setup (extracted hook)
  const { shouldVirtualize, rowVirtualizer, ensureGroupVisible } = useVirtualChatList({
    conversation,
    scrollContainerRef,
    virtualizationThreshold: VIRTUALIZATION_THRESHOLD,
    estimatedItemHeight: ESTIMATED_CHAT_ITEM_HEIGHT,
  });

  const setSearchQueryForTab = useCallback(
    (query: string): void => {
      setSearchQuery(query, conversation);
    },
    [setSearchQuery, conversation]
  );

  // Sticky context button height (py-3 = 12px padding * 2 + button height ~28px + pt-3 = 12px)
  // Total: approximately 52px, round up to 60px for safety
  const STICKY_BUTTON_OFFSET = allContextInjections.length > 0 ? 60 : 0;

  // Unified navigation controller - replaces useNavigationCoordinator + useSearchContextNavigation
  // Must be created before useAutoScrollBottom so we can pass shouldDisableAutoScroll
  const {
    highlightedGroupId,
    setHighlightedGroupId,
    highlightToolUseId: controllerToolUseId,
    isSearchHighlight,
    highlightColor,
    shouldDisableAutoScroll,
  } = useTabNavigationController({
    isActiveTab: isThisTabActive,
    pendingNavigation,
    conversation,
    conversationLoading,
    consumeTabNavigation,
    tabId: effectiveTabId ?? '',
    aiGroupRefs,
    chatItemRefs,
    toolItemRefs,
    expandAIGroup,
    expandSubagentTrace,
    scrollContainerRef,
    stickyOffset: STICKY_BUTTON_OFFSET,
    ensureGroupVisible,
    setSearchQuery: setSearchQueryForTab,
    selectSearchMatch,
  });

  // Local tool highlight for context panel navigation (separate from controller)
  const [contextNavToolUseId, setContextNavToolUseId] = useState<string | null>(null);
  const effectiveHighlightToolUseId = controllerToolUseId ?? contextNavToolUseId ?? undefined;
  // Use blue for context panel tool navigation, otherwise use controller's color
  const effectiveHighlightColor = contextNavToolUseId ? ('blue' as const) : (highlightColor ?? 'blue');

  // Keep search match indices aligned with this tab's rendered conversation.
  // This avoids stale/global match lists after tab switches or in-place refreshes.
  useEffect(() => {
    if (!isThisTabActive || !searchQuery.trim()) {
      return;
    }
    setSearchQuery(searchQuery, conversation);
  }, [isThisTabActive, searchQuery, conversation, setSearchQuery]);

  // Canonicalize matches from rendered mark elements (DOM order).
  // This guarantees that nth navigation follows the exact nth visible highlight.
  // Skip when virtualizing: only a subset of items are rendered, so DOM-based sync
  // would produce an incomplete match list. The store-level matches are already correct.
  useEffect(() => {
    if (!isThisTabActive || !isSearchActive || !conversation || shouldVirtualize) {
      emptyRenderedSyncCountRef.current = 0;
      return;
    }

    let frameA = 0;
    let frameB = 0;
    let cancelled = false;

    const run = (): void => {
      const container = scrollContainerRef.current;
      if (!container || cancelled) return;

      const renderedMatches: { itemId: string; matchIndexInItem: number }[] = [];
      const marks = container.querySelectorAll<HTMLElement>(
        'mark[data-search-item-id][data-search-match-index]'
      );
      for (const mark of marks) {
        const itemId = mark.dataset.searchItemId;
        const matchIndexRaw = mark.dataset.searchMatchIndex;
        const matchIndex = matchIndexRaw !== undefined ? Number(matchIndexRaw) : Number.NaN;
        if (!itemId || !Number.isFinite(matchIndex)) continue;
        renderedMatches.push({ itemId, matchIndexInItem: matchIndex });
      }

      // Prevent transient "0 marks" snapshots during mount from wiping results.
      if (renderedMatches.length === 0 && searchMatches.length > 0) {
        emptyRenderedSyncCountRef.current += 1;
        if (emptyRenderedSyncCountRef.current < 3) {
          return;
        }
      } else {
        emptyRenderedSyncCountRef.current = 0;
      }

      syncSearchMatchesWithRendered(renderedMatches);
    };

    // Wait for highlight marks to be mounted and stabilized.
    frameA = requestAnimationFrame(() => {
      frameB = requestAnimationFrame(run);
    });

    return () => {
      cancelled = true;
      cancelAnimationFrame(frameA);
      cancelAnimationFrame(frameB);
    };
  }, [
    isThisTabActive,
    isSearchActive,
    shouldVirtualize,
    conversation,
    currentSearchIndex,
    searchMatches,
    syncSearchMatchesWithRendered,
  ]);

  // Track shouldDisableAutoScroll transitions for scroll restore coordination
  const prevShouldDisableRef = useRef(shouldDisableAutoScroll);

  const { registerAIGroupRef } = useVisibleAIGroup({
    onVisibleChange: (aiGroupId) => {
      if (effectiveTabId) {
        setTabVisibleAIGroup(effectiveTabId, aiGroupId);
      }
    },
    threshold: 0.5,
    rootRef: scrollContainerRef,
  });

  // Auto-follow when conversation updates, but only if the user was already near bottom.
  // This preserves manual reading position when the user scrolls up.
  // Disabled during navigation to prevent conflicts with deep-link/search scrolling.
  useAutoScrollBottom([conversation], {
    threshold: 150,
    smoothDuration: 300,
    autoBehavior: 'auto',
    disabled: shouldDisableAutoScroll,
    externalRef: scrollContainerRef,
    resetKey: effectiveTabId,
  });

  // Callback to register AI group refs (combines with visibility hook)
  const registerAIGroupRefCombined = useCallback(
    (groupId: string) => {
      const visibilityRef = registerAIGroupRef(groupId);
      return (el: HTMLElement | null) => {
        if (typeof visibilityRef === 'function') visibilityRef(el);
        if (el) aiGroupRefs.current.set(groupId, el);
        else aiGroupRefs.current.delete(groupId);
      };
    },
    [registerAIGroupRef]
  );

  // Handler to navigate to a specific turn (AI group) from CLAUDE.md panel
  const handleNavigateToTurn = useCallback(
    (turnIndex: number) => {
      if (!conversation) return;
      const targetItem = conversation.items.find(
        (item) => item.type === 'ai' && item.group.turnIndex === turnIndex
      );
      if (targetItem?.type !== 'ai') return;

      const run = async (): Promise<void> => {
        const groupId = targetItem.group.id;
        await ensureGroupVisible(groupId);
        const element = aiGroupRefs.current.get(groupId);
        if (!element) return;

        element.scrollIntoView({ behavior: 'smooth', block: 'center' });
        setHighlightedGroupId(groupId);
        setIsNavigationHighlight(true);
        if (navigationHighlightTimerRef.current) {
          clearTimeout(navigationHighlightTimerRef.current);
        }
        navigationHighlightTimerRef.current = setTimeout(() => {
          setHighlightedGroupId(null);
          setIsNavigationHighlight(false);
          navigationHighlightTimerRef.current = null;
        }, 2000);
      };
      void run();
    },
    [conversation, ensureGroupVisible, setHighlightedGroupId]
  );

  // Handler to navigate to a user message group (preceding the AI group at turnIndex)
  const handleNavigateToUserGroup = useCallback(
    (turnIndex: number) => {
      if (!conversation) return;
      const aiItemIndex = conversation.items.findIndex(
        (item) => item.type === 'ai' && item.group.turnIndex === turnIndex
      );
      if (aiItemIndex < 0) return;

      // Find the user item preceding this AI group
      const prevItem = aiItemIndex > 0 ? conversation.items[aiItemIndex - 1] : null;
      if (prevItem?.type !== 'user') return;

      const run = async (): Promise<void> => {
        const groupId = prevItem.group.id;
        await ensureGroupVisible(groupId);
        const element = chatItemRefs.current.get(groupId);
        if (!element) return;

        element.scrollIntoView({ behavior: 'smooth', block: 'center' });
        setHighlightedGroupId(groupId);
        setIsNavigationHighlight(true);
        if (navigationHighlightTimerRef.current) {
          clearTimeout(navigationHighlightTimerRef.current);
        }
        navigationHighlightTimerRef.current = setTimeout(() => {
          setHighlightedGroupId(null);
          setIsNavigationHighlight(false);
          navigationHighlightTimerRef.current = null;
        }, 2000);
      };
      void run();
    },
    [conversation, ensureGroupVisible, setHighlightedGroupId]
  );

  // Handler to navigate to a specific tool within a turn from context panel
  const handleNavigateToTool = useCallback(
    (turnIndex: number, toolUseId: string) => {
      if (!conversation) return;
      const targetItem = conversation.items.find(
        (item) => item.type === 'ai' && item.group.turnIndex === turnIndex
      );
      if (targetItem?.type !== 'ai') return;

      const run = async (): Promise<void> => {
        const groupId = targetItem.group.id;
        await ensureGroupVisible(groupId);

        // Set group + tool highlight immediately
        setHighlightedGroupId(groupId);
        setIsNavigationHighlight(true);
        setContextNavToolUseId(toolUseId);

        // Wait for tool element to appear in DOM (up to 500ms)
        let toolElement: HTMLElement | undefined;
        const startTime = Date.now();
        while (Date.now() - startTime < 500) {
          toolElement = toolItemRefs.current.get(toolUseId);
          if (toolElement) break;
          await new Promise((resolve) => setTimeout(resolve, 50));
        }

        // Scroll to tool element, or fall back to AI group
        const scrollTarget = toolElement ?? aiGroupRefs.current.get(groupId);
        if (scrollTarget) {
          scrollTarget.scrollIntoView({ behavior: 'smooth', block: 'center' });
        }

        // Clear highlight after 2s
        if (navigationHighlightTimerRef.current) {
          clearTimeout(navigationHighlightTimerRef.current);
        }
        navigationHighlightTimerRef.current = setTimeout(() => {
          setHighlightedGroupId(null);
          setIsNavigationHighlight(false);
          setContextNavToolUseId(null);
          navigationHighlightTimerRef.current = null;
        }, 2000);
      };
      void run();
    },
    [conversation, ensureGroupVisible, setHighlightedGroupId]
  );

  // Scroll to current search result when it changes
  useEffect(() => {
    const currentMatch = currentSearchIndex >= 0 ? searchMatches[currentSearchIndex] : null;
    if (!currentMatch) return;

    let frameId = 0;
    let attempt = 0;
    let cancelled = false;

    /**
     * Promote a mark element to "current" (demote any previous) and scroll to it.
     */
    const promoteAndScroll = (el: HTMLElement): void => {
      const container = scrollContainerRef.current;
      if (container) {
        container
          .querySelectorAll<HTMLElement>('mark[data-search-result="current"]')
          .forEach((prev) => {
            /* eslint-disable no-param-reassign -- Directly mutating DOM element style/attributes is necessary for search result highlighting */
            prev.setAttribute('data-search-result', 'match');
            prev.style.backgroundColor = 'var(--highlight-bg-inactive)';
            prev.style.color = 'var(--highlight-text-inactive)';
            prev.style.boxShadow = '';
            /* eslint-enable no-param-reassign -- Re-enable after DOM mutations */
          });
      }
      /* eslint-disable no-param-reassign -- Directly mutating DOM element style/attributes is necessary for current search result highlighting */
      el.setAttribute('data-search-result', 'current');
      el.style.backgroundColor = 'var(--highlight-bg)';
      el.style.color = 'var(--highlight-text)';
      el.style.boxShadow = '0 0 0 1px var(--highlight-ring)';
      /* eslint-enable no-param-reassign -- Re-enable after DOM mutations */
      el.scrollIntoView({ behavior: 'smooth', block: 'center' });
    };

    /**
     * DOM text-search fallback: walk text nodes inside the group element to find the
     * Nth occurrence of the search query, then scroll the enclosing element into view.
     * This works even when React hasn't created <mark> elements (ReactMarkdown
     * component memoization, render timing, etc.).
     */
    const fallbackDOMSearch = (): boolean => {
      const groupEl =
        chatItemRefs.current.get(currentMatch.itemId) ??
        aiGroupRefs.current.get(currentMatch.itemId);
      if (!groupEl) return false;

      const query = useStore.getState().searchQuery;
      if (!query) return false;
      const lowerQuery = query.toLowerCase();
      let count = 0;

      // Scope to [data-search-content] elements to exclude UI chrome
      // (timestamps, labels, buttons) from text-node walking
      const searchRoots = groupEl.querySelectorAll<HTMLElement>('[data-search-content]');
      const roots = searchRoots.length > 0 ? Array.from(searchRoots) : [groupEl];

      for (const root of roots) {
        const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
        let node: Node | null;
        while ((node = walker.nextNode())) {
          const text = node.textContent ?? '';
          const lowerText = text.toLowerCase();
          let pos = 0;
          while ((pos = lowerText.indexOf(lowerQuery, pos)) !== -1) {
            if (count === currentMatch.matchIndexInItem) {
              const parent = node.parentElement;
              if (parent) {
                parent.scrollIntoView({ behavior: 'smooth', block: 'center' });
                return true;
              }
            }
            count++;
            pos += lowerQuery.length;
          }
        }
      }
      return false;
    };

    const tryScrollToResult = (): void => {
      const container = scrollContainerRef.current;
      if (!container) return;

      // Primary: find mark by item ID + match index
      const el = container.querySelector<HTMLElement>(
        `mark[data-search-item-id="${CSS.escape(currentMatch.itemId)}"][data-search-match-index="${currentMatch.matchIndexInItem}"]`
      );
      if (el) {
        promoteAndScroll(el);
        return;
      }

      // Secondary: align by global order (nth rendered mark) as canonical fallback.
      if (attempt >= 3) {
        const orderedMarks = Array.from(
          container.querySelectorAll<HTMLElement>(
            'mark[data-search-item-id][data-search-match-index]'
          )
        );
        const byGlobal = orderedMarks[currentSearchIndex];
        if (byGlobal) {
          promoteAndScroll(byGlobal);
          return;
        }
      }

      // After a few frames, try fallback DOM text search
      if (attempt >= 6) {
        if (fallbackDOMSearch()) return;
      }

      // Keep retrying (marks may appear after async render)
      if (attempt < 60) {
        attempt++;
        frameId = requestAnimationFrame(tryScrollToResult);
      }
    };

    const run = async (): Promise<void> => {
      await ensureGroupVisible(currentMatch.itemId);
      if (cancelled) return;
      frameId = requestAnimationFrame(tryScrollToResult);
    };

    void run();
    return () => {
      cancelled = true;
      cancelAnimationFrame(frameId);
    };
  }, [currentSearchIndex, searchMatches, scrollContainerRef, ensureGroupVisible]);

  // Track previous active state to detect when THIS tab becomes active/inactive
  const wasActiveRef = useRef(isThisTabActive);

  // Save scroll position when THIS tab becomes inactive
  useEffect(() => {
    const wasActive = wasActiveRef.current;
    wasActiveRef.current = isThisTabActive;

    // If this tab just became inactive, save its scroll position
    if (wasActive && !isThisTabActive && scrollContainerRef.current) {
      saveScrollPosition(scrollContainerRef.current.scrollTop);
    }
  }, [isThisTabActive, saveScrollPosition, scrollContainerRef]);

  // Also save on unmount (e.g., when tab is closed)
  useEffect(() => {
    const scrollContainer = scrollContainerRef.current;
    return () => {
      if (scrollContainer) {
        saveScrollPosition(scrollContainer.scrollTop);
      }
    };
  }, [saveScrollPosition, scrollContainerRef]);

  // Restore scroll position when THIS tab becomes active with saved position
  // Uses shouldDisableAutoScroll (covers full navigation lifecycle) instead of pendingNavigation
  // After navigation completes (transition true→false), save current position to prevent stale restore
  useEffect(() => {
    const wasDisabled = prevShouldDisableRef.current;
    prevShouldDisableRef.current = shouldDisableAutoScroll;

    // Navigation just completed — save current scroll position, skip restore
    if (wasDisabled && !shouldDisableAutoScroll && scrollContainerRef.current) {
      saveScrollPosition(scrollContainerRef.current.scrollTop);
      return;
    }

    if (
      isThisTabActive &&
      savedScrollTop !== undefined &&
      scrollContainerRef.current &&
      !conversationLoading &&
      !shouldDisableAutoScroll
    ) {
      let frameA = 0;
      let frameB = 0;
      // Use double RAF so layout + virtual rows settle before restore.
      frameA = requestAnimationFrame(() => {
        frameB = requestAnimationFrame(() => {
          if (scrollContainerRef.current) {
            scrollContainerRef.current.scrollTop = savedScrollTop;
          }
        });
      });
      return () => {
        cancelAnimationFrame(frameA);
        cancelAnimationFrame(frameB);
      };
    }
  }, [
    isThisTabActive,
    savedScrollTop,
    conversationLoading,
    scrollContainerRef,
    shouldDisableAutoScroll,
    saveScrollPosition,
  ]);

  useEffect(() => {
    return () => {
      if (navigationHighlightTimerRef.current) {
        clearTimeout(navigationHighlightTimerRef.current);
      }
    };
  }, []);

  // Register ref for user/system chat items
  const registerChatItemRef = useCallback((groupId: string) => {
    return (el: HTMLElement | null) => {
      if (el) chatItemRefs.current.set(groupId, el);
      else chatItemRefs.current.delete(groupId);
    };
  }, []);

  // Register ref for individual tool items (for precise scroll targeting)
  const registerToolRef = useCallback((toolId: string, el: HTMLElement | null) => {
    if (el) toolItemRefs.current.set(toolId, el);
    else toolItemRefs.current.delete(toolId);
  }, []);

  // Plan chain banners
  const currentSession = sessionDetail?.session;
  const planChainMap = useMemo(() => buildPlanChainMap(sessions), [sessions]);

  const navigatePlanChain = useCallback(
    (sessionId: string, scrollTo: 'top' | 'bottom', event: React.MouseEvent) => {
      if (!activeProjectId) return;
      const forceNewTab = event.ctrlKey || event.metaKey;
      pendingPlanChainScroll = scrollTo;
      openTab(
        {
          type: 'session' as const,
          sessionId,
          projectId: activeProjectId,
          label: sessionId.slice(0, 8),
        },
        forceNewTab ? { forceNewTab } : { replaceActiveTab: true }
      );
      selectSession(sessionId);
    },
    [activeProjectId, sessions, openTab, selectSession]
  );

  // Execute pending plan chain scroll after conversation loads in the new tab.
  // The module-level variable survives the unmount/remount caused by tab key change.
  useEffect(() => {
    if (!pendingPlanChainScroll || conversationLoading || !conversation) return;
    const scrollTo = pendingPlanChainScroll;
    pendingPlanChainScroll = null;

    setTimeout(() => {
      requestAnimationFrame(() => {
        const el = scrollContainerRef.current;
        if (!el) return;
        el.scrollTop = scrollTo === 'bottom' ? el.scrollHeight : 0;
      });
    }, 100);
  }, [conversationLoading, conversation, scrollContainerRef]);

  // Plan chain link context menu state
  const [planChainMenu, setPlanChainMenu] = useState<{
    x: number;
    y: number;
    sessionId: string;
    scrollTo: 'top' | 'bottom';
  } | null>(null);

  const handlePlanChainContextMenu = useCallback(
    (e: React.MouseEvent, sessionId: string, scrollTo: 'top' | 'bottom') => {
      e.preventDefault();
      setPlanChainMenu({ x: e.clientX, y: e.clientY, sessionId, scrollTo });
    },
    []
  );

  const handlePlanChainOpen = useCallback(
    (sessionId: string, scrollTo: 'top' | 'bottom', mode: 'current' | 'newTab' | 'splitRight') => {
      if (!activeProjectId) return;
      pendingPlanChainScroll = scrollTo;
      const tabDef = {
        type: 'session' as const,
        sessionId,
        projectId: activeProjectId,
        label: sessionId.slice(0, 8),
      };
      if (mode === 'newTab') {
        openTab(tabDef, { forceNewTab: true });
      } else if (mode === 'splitRight') {
        openTab(tabDef);
        selectSession(sessionId);
        const state = useStore.getState();
        const focusedPaneId = state.paneLayout.focusedPaneId;
        const currentActiveTabId = state.activeTabId;
        if (currentActiveTabId) {
          splitPane(focusedPaneId, currentActiveTabId, 'right');
        }
        return;
      } else {
        openTab(tabDef, { replaceActiveTab: true });
      }
      selectSession(sessionId);
    },
    [activeProjectId, openTab, selectSession, splitPane]
  );

  const planChainBanners = useMemo(() => {
    if (!currentSession?.id) return { top: null, bottom: null };
    const link = planChainMap.get(currentSession.id);
    if (!link) return { top: null, bottom: null };

    const shortId = (id: string) => id.slice(0, 8);

    let top: React.ReactNode = null;
    let bottom: React.ReactNode = null;

    // This session was continued from a plan
    if (link.prevSessionId) {
      // Find the planContent from the first user message in the conversation
      const firstUserWithPlan = conversation?.items.find(
        (item): item is { type: 'user'; group: { content: { planContent?: string } } } & typeof item =>
          item.type === 'user' && Boolean(item.group.content.planContent)
      );
      const planContent = firstUserWithPlan?.group.content.planContent;

      if (planContent) {
        // Full plan card with scrollable markdown
        top = (
          <div
            className="overflow-hidden rounded-lg"
            style={{
              backgroundColor: 'var(--plan-exit-bg)',
              border: '1px solid var(--plan-exit-border)',
            }}
          >
            {/* Header */}
            <div
              className="flex items-center justify-between px-4 py-2"
              style={{
                borderBottom: '1px solid var(--plan-exit-border)',
                backgroundColor: 'var(--plan-exit-header-bg)',
              }}
            >
              <div className="flex items-center gap-2">
                <FileCheck className="size-4" style={{ color: 'var(--plan-exit-text)' }} />
                <span className="text-sm font-medium" style={{ color: 'var(--plan-exit-text)' }}>
                  Implementation Plan
                </span>
                <span className="text-xs" style={{ color: 'var(--plan-exit-text)', opacity: 0.6 }}>
                  from
                </span>
                <button
                  onClick={(e) => navigatePlanChain(link.prevSessionId!, 'bottom', e)}
                  onContextMenu={(e) => handlePlanChainContextMenu(e, link.prevSessionId!, 'bottom')}
                  className="font-mono text-xs underline decoration-dotted underline-offset-2 transition-colors hover:opacity-80"
                  style={{ color: 'var(--plan-exit-text)' }}
                >
                  {shortId(link.prevSessionId)}
                </button>
              </div>
              <CopyButton text={planContent} inline />
            </div>

            {/* Plan content - scrollable */}
            <div className="max-h-96 overflow-y-auto px-4 py-3">
              <ReactMarkdown remarkPlugins={[remarkGfm]} components={markdownComponents}>
                {planContent}
              </ReactMarkdown>
            </div>
          </div>
        );
      } else {
        // Fallback: simple link banner (no planContent available yet)
        top = (
          <div
            className="flex items-center gap-2 rounded-lg px-4 py-2.5 text-xs"
            style={{
              backgroundColor: 'var(--color-surface-raised)',
              border: '1px solid var(--color-border)',
              color: 'var(--color-text-secondary)',
            }}
          >
            <span className="shrink-0" style={{ color: 'var(--color-text-muted)' }}>Continued from plan in</span>
            <button
              onClick={(e) => navigatePlanChain(link.prevSessionId!, 'bottom', e)}
              onContextMenu={(e) => handlePlanChainContextMenu(e, link.prevSessionId!, 'bottom')}
              className="font-mono underline decoration-dotted underline-offset-2 transition-colors hover:opacity-80"
              style={{ color: 'var(--color-accent)' }}
            >
              {shortId(link.prevSessionId)}
            </button>
          </div>
        );
      }
    }

    // This session has an implementation continuation → show banner at bottom
    if (link.nextSessionId) {
      bottom = (
        <div
          className="flex items-center gap-2 rounded-lg px-4 py-2.5 text-xs"
          style={{
            backgroundColor: 'var(--color-surface-raised)',
            border: '1px solid var(--color-border)',
            color: 'var(--color-text-secondary)',
          }}
        >
          <span className="shrink-0" style={{ color: 'var(--color-text-muted)' }}>Implementation continues in</span>
          <button
            onClick={(e) => navigatePlanChain(link.nextSessionId!, 'top', e)}
            onContextMenu={(e) => handlePlanChainContextMenu(e, link.nextSessionId!, 'top')}
            className="font-mono underline decoration-dotted underline-offset-2 transition-colors hover:opacity-80"
            style={{ color: 'var(--color-accent)' }}
          >
            {shortId(link.nextSessionId)}
          </button>
        </div>
      );
    }

    return { top, bottom };
  }, [currentSession, planChainMap, conversation, navigatePlanChain, handlePlanChainContextMenu]);

  // Loading state
  if (conversationLoading) return <ChatHistoryLoadingState />;

  // Empty state
  if (!conversation || conversation.items.length === 0) return <ChatHistoryEmptyState />;

  return (
    <div
      className="flex flex-1 flex-col overflow-hidden"
      style={{ backgroundColor: 'var(--color-surface)' }}
    >
      <div className="flex flex-1 overflow-hidden">
        {/* Chat content */}
        <ChatScrollContainer
          scrollContainerRef={scrollContainerRef}
          conversation={conversation}
          shouldVirtualize={shouldVirtualize}
          topBanner={planChainBanners.top}
          bottomBanner={planChainBanners.bottom}
          rowVirtualizer={rowVirtualizer}
          allContextInjections={allContextInjections}
          isContextPanelVisible={isContextPanelVisible}
          setContextPanelVisible={setContextPanelVisible}
          isContextButtonHovered={isContextButtonHovered}
          setIsContextButtonHovered={setIsContextButtonHovered}
          highlightedGroupId={highlightedGroupId}
          effectiveHighlightToolUseId={effectiveHighlightToolUseId}
          isSearchHighlight={isSearchHighlight}
          isNavigationHighlight={isNavigationHighlight}
          effectiveHighlightColor={effectiveHighlightColor}
          registerChatItemRef={registerChatItemRef}
          registerAIGroupRefCombined={registerAIGroupRefCombined}
          registerToolRef={registerToolRef}
        />

        {/* Context panel sidebar */}
        {isContextPanelVisible && allContextInjections.length > 0 && (
          <div className={`w-80 shrink-0 transition-opacity duration-200 ${isContextStale ? 'opacity-50' : ''}`}>
            <SessionContextPanel
              injections={allContextInjections}
              onClose={() => setContextPanelVisible(false)}
              projectRoot={sessionDetail?.session?.projectPath}
              onNavigateToTurn={handleNavigateToTurn}
              onNavigateToTool={handleNavigateToTool}
              onNavigateToUserGroup={handleNavigateToUserGroup}
              totalSessionTokens={lastAiGroupTotalTokens}
              phaseInfo={sessionPhaseInfo ?? undefined}
              selectedPhase={selectedContextPhase}
              onPhaseChange={setSelectedContextPhase}
            />
          </div>
        )}
      </div>

      {planChainMenu &&
        createPortal(
          <PlanChainLinkMenu
            x={planChainMenu.x}
            y={planChainMenu.y}
            paneCount={paneCount}
            onClose={() => setPlanChainMenu(null)}
            onOpenInCurrentPane={() => handlePlanChainOpen(planChainMenu.sessionId, planChainMenu.scrollTo, 'current')}
            onOpenInNewTab={() => handlePlanChainOpen(planChainMenu.sessionId, planChainMenu.scrollTo, 'newTab')}
            onSplitRightAndOpen={() => handlePlanChainOpen(planChainMenu.sessionId, planChainMenu.scrollTo, 'splitRight')}
          />,
          document.body
        )}
    </div>
  );
};

// =============================================================================
// Plan chain link context menu (navigation only, no pin/hide)
// =============================================================================

const PlanChainLinkMenu = ({
  x,
  y,
  paneCount,
  onClose,
  onOpenInCurrentPane,
  onOpenInNewTab,
  onSplitRightAndOpen,
}: Readonly<{
  x: number;
  y: number;
  paneCount: number;
  onClose: () => void;
  onOpenInCurrentPane: () => void;
  onOpenInNewTab: () => void;
  onSplitRightAndOpen: () => void;
}>): React.JSX.Element => {
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleMouseDown = (e: MouseEvent): void => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    const handleKeyDown = (e: KeyboardEvent): void => {
      if (e.key === 'Escape') onClose();
    };
    document.addEventListener('mousedown', handleMouseDown);
    document.addEventListener('keydown', handleKeyDown);
    return () => {
      document.removeEventListener('mousedown', handleMouseDown);
      document.removeEventListener('keydown', handleKeyDown);
    };
  }, [onClose]);

  const menuWidth = 240;
  const menuHeight = 120;
  const clampedX = Math.min(x, window.innerWidth - menuWidth - 8);
  const clampedY = Math.min(y, window.innerHeight - menuHeight - 8);

  const handleClick = (action: () => void) => () => {
    action();
    onClose();
  };

  const atMaxPanes = paneCount >= MAX_PANES;

  return (
    <div
      ref={menuRef}
      className="fixed z-50 min-w-[220px] overflow-hidden rounded-md border py-1 shadow-lg"
      style={{
        left: clampedX,
        top: clampedY,
        backgroundColor: 'var(--color-surface-overlay)',
        borderColor: 'var(--color-border-emphasis)',
        color: 'var(--color-text)',
      }}
    >
      <button
        className="flex w-full items-center justify-between px-3 py-1.5 text-left text-sm transition-colors hover:bg-[var(--color-surface-raised)]"
        onClick={handleClick(onOpenInCurrentPane)}
      >
        Open in Current Pane
      </button>
      <button
        className="flex w-full items-center justify-between px-3 py-1.5 text-left text-sm transition-colors hover:bg-[var(--color-surface-raised)]"
        onClick={handleClick(onOpenInNewTab)}
      >
        <span>Open in New Tab</span>
        <span className="ml-4 text-xs" style={{ color: 'var(--color-text-muted)' }}>
          ⌘ Click
        </span>
      </button>
      <div className="mx-2 my-1 border-t" style={{ borderColor: 'var(--color-border)' }} />
      <button
        className="flex w-full items-center justify-between px-3 py-1.5 text-left text-sm transition-colors hover:bg-[var(--color-surface-raised)]"
        onClick={handleClick(onSplitRightAndOpen)}
        disabled={atMaxPanes}
        style={{ opacity: atMaxPanes ? 0.4 : 1 }}
      >
        Split Right and Open
      </button>
    </div>
  );
};
