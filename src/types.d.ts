import { DISPLAY_TYPE } from 'src/constants';

export type MonitorUpdateInput = {
  id: string;
  name?: string;
  brightness?: number;
  disabled?: boolean;
  sortOrder?: number;
  type: DISPLAY_TYPE;
};

export type Monitor = Required<MonitorUpdateInput>;
