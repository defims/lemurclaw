import '@testing-library/jest-dom/vitest';

// jsdom does not implement Element.scrollIntoView (it's a layout-dependent
// method with no meaningful behavior in a headless DOM). Components like
// Scrollback call it on mount to stick to the bottom; stub it as a no-op so
// integration tests that render the full tree don't throw.
if (typeof Element !== 'undefined' && !Element.prototype.scrollIntoView) {
  Element.prototype.scrollIntoView = function scrollIntoView() {};
}
