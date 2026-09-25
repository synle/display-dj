import type { SelectHTMLAttributes } from 'react';

type DropdownProps = SelectHTMLAttributes<HTMLSelectElement>;

/** Native select with shared dropdown sizing, spacing, and interaction styles. */
export default function Dropdown({ className, ...props }: DropdownProps) {
  const classes = ['dropdown', className].filter(Boolean).join(' ');
  return <select className={classes} {...props} />;
}
