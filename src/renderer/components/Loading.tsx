import { CSSProperties } from 'react';
import CircularProgress from '@mui/material/CircularProgress';

type LoadingProps = {
  className?: string;
  style?: CSSProperties;
};

export function Loading(props: LoadingProps) {
  let { className, style } = props;
  className = className || '';
  className += ' loading';
  className = className.trim();

  return <div className={className} style={style}>
    <CircularProgress  size={20}/>
  </div>;
}

export default Loading;
