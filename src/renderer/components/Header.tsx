import { ToggleAllDisplay } from 'src/renderer/components/ToggleAllDisplay';
import { AppConfig, Preference } from 'src/types.d';
import { LAPTOP_BUILT_IN_DISPLAY_ID } from 'src/constants';
import {
  useBatchUpdateMonitors,
  usePreferences,
  useUpdatePreferences,
  useConfigs,
  useUpdateMonitor,
  useToggleDarkMode,
  useUpdateAppPosition,
  useUpdateVolume,
  QUERY_KEY_CONFIGS,
  QUERY_KEY_APP_STATE,
} from 'src/renderer/hooks';

type HeaderProps = {
  configs?: AppConfig;
  preference?: Preference;
};

export function Header(props: HeaderProps) {
  const { configs, preference } = props;

  return (
    <header>
      <h2>
        Display-DJ {configs?.version} {configs?.env !== 'production' ? configs?.env : ''}
      </h2>
      {!preference ? null : <ToggleAllDisplay preference={preference} />}
    </header>
  );
}
