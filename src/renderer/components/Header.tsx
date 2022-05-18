import { ToggleAllDisplay } from 'src/renderer/components/ToggleAllDisplay';
import { AppConfig, Preference } from 'src/types.d';
import Typography from '@mui/material/Typography';

type HeaderProps = {
  configs?: AppConfig;
  preference?: Preference;
};

export function Header(props: HeaderProps) {
  const { configs, preference } = props;

  return (
    <header>
      <Typography variant="h5" className='flexAlignItems'>
        Display-DJ {configs?.version} {configs?.env !== 'production' ? configs?.env : ''}
      </Typography>
      {!preference ? null : <ToggleAllDisplay preference={preference} />}
    </header>
  );
}
