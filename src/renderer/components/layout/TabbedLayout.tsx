/**
 * TabbedLayout - Main layout with project-centric sidebar and multi-pane tabbed content.
 *
 * Layout structure:
 * - Sidebar (280px): Project dropdown + date-grouped sessions
 * - Main content: PaneContainer with one or more panes, each with TabBar + content
 */

import { useKeyboardShortcuts } from '@renderer/hooks/useKeyboardShortcuts';

import { CommandPalette } from '../search/CommandPalette';

import { PaneContainer } from './PaneContainer';
import { Sidebar } from './Sidebar';

export const TabbedLayout = (): React.JSX.Element => {
  // Enable keyboard shortcuts
  useKeyboardShortcuts();

  return (
    <div className="flex h-screen flex-col bg-claude-dark-bg text-claude-dark-text">
      <div className="flex flex-1 overflow-hidden">
        {/* Command Palette (Cmd+K) */}
        <CommandPalette />

        {/* Sidebar - Project dropdown + Sessions (280px) */}
        <Sidebar />

        {/* Multi-pane content area */}
        <PaneContainer />
      </div>
    </div>
  );
};
