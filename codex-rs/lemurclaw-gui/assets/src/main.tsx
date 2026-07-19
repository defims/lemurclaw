import { createRoot } from 'react-dom/client';
import './styles.css';
import { App } from './app/App';

const rootEl = document.getElementById('root');
if (!rootEl) {
  throw new Error('lemurclaw-gui: #root element not found in index.html');
}
createRoot(rootEl).render(<App />);
