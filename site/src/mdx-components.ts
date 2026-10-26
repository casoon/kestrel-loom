import Diagram from './components/Diagram.astro';
import Fixture from './components/Fixture.astro';

// Components for docs/ MDX, which cannot import anything itself.
export const components = { Diagram, Fixture };
