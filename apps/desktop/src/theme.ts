import { createTheme } from '@mantine/core';

export const theme = createTheme({
  primaryColor: 'blue',
  defaultRadius: 'md',
  fontFamily:
    '-apple-system, BlinkMacSystemFont, "Segoe UI", "Hiragino Sans", "Hiragino Kaku Gothic ProN", "Noto Sans JP", "Yu Gothic UI", Meiryo, sans-serif',
  fontFamilyMonospace: 'ui-monospace, "SF Mono", "Cascadia Mono", Menlo, Consolas, monospace',
  headings: { fontWeight: '600' },
  fontSizes: { xs: '11px', sm: '13px', md: '14px', lg: '16px', xl: '20px' },
  components: {
    Button: { defaultProps: { size: 'xs' } },
    ActionIcon: { defaultProps: { variant: 'subtle', color: 'gray' } },
    Tooltip: { defaultProps: { openDelay: 400, withArrow: true, fz: 'xs' } },
    TextInput: { defaultProps: { size: 'xs' } },
    NumberInput: { defaultProps: { size: 'xs' } },
    Select: { defaultProps: { size: 'xs', allowDeselect: false } },
    SegmentedControl: { defaultProps: { size: 'xs' } },
    Switch: { defaultProps: { size: 'xs' } },
    Tabs: { defaultProps: { variant: 'default' } },
  },
});
