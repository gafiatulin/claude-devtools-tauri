/**
 * GeneralSection - General settings including appearance.
 */

import { SettingRow, SettingsSectionHeader, SettingsSelect } from '../components';

import type { SafeConfig } from '../hooks/useSettingsConfig';

// Theme options
const THEME_OPTIONS = [
  { value: 'dark', label: 'Dark' },
  { value: 'light', label: 'Light' },
  { value: 'system', label: 'System' },
] as const;

interface GeneralSectionProps {
  readonly safeConfig: SafeConfig;
  readonly saving: boolean;
  readonly onGeneralToggle: (key: 'launchAtLogin' | 'showDockIcon', value: boolean) => void;
  readonly onThemeChange: (value: 'dark' | 'light' | 'system') => void;
}

export const GeneralSection = ({
  safeConfig,
  saving,
  onThemeChange,
}: GeneralSectionProps): React.JSX.Element => {
  return (
    <div>
      <SettingsSectionHeader title="Appearance" />
      <SettingRow label="Theme" description="Choose your preferred color theme">
        <SettingsSelect
          value={safeConfig.general.theme}
          options={THEME_OPTIONS}
          onChange={onThemeChange}
          disabled={saving}
        />
      </SettingRow>
    </div>
  );
};
