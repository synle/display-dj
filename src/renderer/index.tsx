import ReactDOM from 'react-dom';
import { QueryClient, QueryClientProvider } from 'react-query';
import { Home } from 'src/renderer/pages/Home';
import './index.scss';

// render the main app
const appQueryClient = new QueryClient();
ReactDOM.render(
  <QueryClientProvider client={appQueryClient}>
    <Home />
  </QueryClientProvider>,
  document.querySelector('#root'),
);
