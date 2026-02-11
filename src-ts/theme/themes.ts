import { Theme } from './types.js';

export const carrycodeLight: Theme = {
  name: 'carrycode-light',
  colors: {
    primary: '#0969da',
    secondary: '#0969da',
    success: '#3fb950',
    error: '#cf222e',
    warning: '#9a6700',
    info: '#0969da',
    text: '#c9d1d9',
    dimText: '#6e7781',
    border: '#d0d7de',
    highlight: '#0969da',
    background: '#ffffff',
    diffAddFg: '#1a7f37',
    diffAddBg: '#cefbd0',
    diffRemFg: '#d1242f',
    diffRemBg: '#ffebe9',
    bannerGradient: 'summer',
  },
};

export const carrycodeDark: Theme = {
  name: 'carrycode-dark',
  colors: {
    primary: '#58a6ff',
    secondary: '#58a6ff',
    success: '#3fb950',
    error: '#f85149',
    warning: '#d29922',
    info: '#58a6ff',
    text: '#c9d1d9',
    dimText: '#8b949e',
    border: '#d0d7de',
    highlight: '#58a6ff',
    background: '#0d1117',
    diffAddFg: '#1b4721',
    diffAddBg: '#cefbd0',
    diffRemFg: '#f85149',
    diffRemBg: '#ffebe9',
    bannerGradient: 'morning',
  },
};

export const themes = {
  carrycodeLight,
  carrycodeDark,
};
