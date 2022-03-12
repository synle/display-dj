export type MonitorUpdateInput = {
  id: string;
  name?: string;
  brightness?: number;
};

export type Monitor = Required<MonitorUpdateInput>;
