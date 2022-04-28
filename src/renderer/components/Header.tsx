import { ToggleAllDisplay } from 'src/renderer/components/ToggleAllDisplay';
import { AppConfig, Preference } from 'src/types.d';

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
