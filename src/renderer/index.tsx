import ReactDOM from 'react-dom';
import { useEffect, useState, useMemo } from 'react';
import { QueryClient, QueryClientProvider } from 'react-query';
import { Home } from 'src/renderer/pages/Home';
import { ThemeProvider, createTheme, ThemeOptions } from '@mui/material/styles';
import useMediaQuery from '@mui/material/useMediaQuery';
import CssBaseline from '@mui/material/CssBaseline';
import './index.scss';

const DisplayDJFrontEndApp = () => {
  // handling and setting theme
  const prefersDarkMode = useMediaQuery('(prefers-color-scheme: dark)');
  const theme = useMemo(
    () =>
      createTheme({
        palette: {
          mode: prefersDarkMode ? 'dark' : 'light',
        },
      }),
    [prefersDarkMode],
  );

  return <ThemeProvider theme={theme}>
  <CssBaseline />
  <Home />
  </ThemeProvider>
}

// render the main app
const appQueryClient = new QueryClient();
ReactDOM.render(
  <QueryClientProvider client={appQueryClient}>
    <DisplayDJFrontEndApp />
  </QueryClientProvider>,
  document.querySelector('#root'),
);
