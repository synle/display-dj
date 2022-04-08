import ReactDOM from 'react-dom';
import { QueryClient, QueryClientProvider } from 'react-query';
import { Home } from 'src/renderer/pages/Home';
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
import './index.scss';

// render the main app
const appQueryClient = new QueryClient();
ReactDOM.render(
  <QueryClientProvider client={appQueryClient}>
    <Home />
  </QueryClientProvider>,
  document.querySelector('#root'),
);
