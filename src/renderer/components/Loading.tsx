import CircularProgress from '@mui/material/CircularProgress';
import { CSSProperties } from 'react';

type LoadingProps = {
  className?: string;
  style?: CSSProperties;
};

export function Loading(props: LoadingProps) {
  let { className, style } = props;
  className = className || '';
  className += ' loading';
  className = className.trim();

  return (
    <div
      className={className}
      style={{
        ...style,
        lineHeight: 'normal',
      }}>
      <CircularProgress size={15} />
    </div>
  );
}

export default Loading;
