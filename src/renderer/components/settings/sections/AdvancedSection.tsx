/**
 * AdvancedSection - Advanced settings including config management and about info.
 */

import { useEffect, useState } from 'react';

import { api } from '@renderer/api';
import appIcon from '@renderer/favicon.png';
import { Download, RefreshCw, Upload } from 'lucide-react';

import { SettingsSectionHeader } from '../components';

interface AdvancedSectionProps {
  readonly saving: boolean;
  readonly onResetToDefaults: () => void;
  readonly onExportConfig: () => void;
  readonly onImportConfig: () => void;
}

export const AdvancedSection = ({
  saving,
  onResetToDefaults,
  onExportConfig,
  onImportConfig,
}: AdvancedSectionProps): React.JSX.Element => {
  const [version, setVersion] = useState<string>('');

  useEffect(() => {
    api.getAppVersion().then(setVersion).catch(console.error);
  }, []);

  return (
    <div>
      <SettingsSectionHeader title="Configuration" />
      <div className="space-y-2 py-2">
        <button
          onClick={onResetToDefaults}
          disabled={saving}
          className={`flex w-full items-center justify-center gap-2 rounded-md border px-4 py-2.5 text-sm font-medium transition-all duration-150 ${saving ? 'cursor-not-allowed opacity-50' : ''} `}
          style={{
            borderColor: 'var(--color-border)',
            color: 'var(--color-text-secondary)',
          }}
        >
          <RefreshCw className="size-4" />
          Reset to Defaults
        </button>
        <button
          onClick={onExportConfig}
          disabled={saving}
          className={`flex w-full items-center justify-center gap-2 rounded-md border px-4 py-2.5 text-sm font-medium transition-all duration-150 ${saving ? 'cursor-not-allowed opacity-50' : ''} `}
          style={{
            borderColor: 'var(--color-border)',
            color: 'var(--color-text-secondary)',
          }}
        >
          <Download className="size-4" />
          Export Config
        </button>
        <button
          onClick={onImportConfig}
          disabled={saving}
          className={`flex w-full items-center justify-center gap-2 rounded-md border px-4 py-2.5 text-sm font-medium transition-all duration-150 ${saving ? 'cursor-not-allowed opacity-50' : ''} `}
          style={{
            borderColor: 'var(--color-border)',
            color: 'var(--color-text-secondary)',
          }}
        >
          <Upload className="size-4" />
          Import Config
        </button>
      </div>

      <SettingsSectionHeader title="About" />
      <div className="flex items-start gap-4 py-3">
        <img src={appIcon} alt="App Icon" className="size-10 rounded-lg" />
        <div>
          <div className="flex items-center gap-3">
            <p className="text-sm font-medium" style={{ color: 'var(--color-text)' }}>
              claude-devtools
            </p>
          </div>
          <p className="mt-0.5 text-xs" style={{ color: 'var(--color-text-muted)' }}>
            Version {version || '...'}
          </p>
          <p className="mt-2 text-xs leading-relaxed" style={{ color: 'var(--color-text-muted)' }}>
            Visualize and analyze Claude Code session executions with interactive waterfall charts
            and detailed insights.
          </p>
        </div>
      </div>
    </div>
  );
};
