import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { ScenarioCardTabs, SecondaryTabs, SegmentedSubTabs } from '../SecondaryTabs';

describe('SecondaryTabs & ScenarioCardTabs', () => {
  const cardItems = [
    {
      value: 'tab1',
      label: 'Tab One',
      description: 'First tab description',
      icon: <span data-testid="icon-1">Icon1</span>,
    },
    {
      value: 'tab2',
      label: 'Tab Two',
      description: 'Second tab description',
      icon: <span data-testid="icon-2">Icon2</span>,
    },
    {
      value: 'tab3',
      label: 'Tab Three',
      description: 'Third tab description',
      disabled: true,
    },
  ];

  it('renders card tabs with accessible roles and content', () => {
    const onChange = vi.fn();
    render(
      <ScenarioCardTabs
        id="test-card-tabs"
        ariaLabel="Test Category"
        items={cardItems}
        activeTab="tab1"
        onChange={onChange}
        idPrefix="my-card-tab"
        getPanelId={(v) => `my-panel-${v}`}
        columns={3}
        bordered
      />
    );

    const tablist = screen.getByRole('tablist', { name: 'Test Category' });
    expect(tablist).toBeDefined();
    expect(tablist.id).toBe('test-card-tabs');
    expect(tablist.classList.contains('three-columns')).toBe(true);
    expect(tablist.classList.contains('is-bordered')).toBe(true);

    const tabs = screen.getAllByRole('tab');
    expect(tabs).toHaveLength(3);

    const tab1 = tabs[0];
    expect(tab1.id).toBe('my-card-tab-tab1');
    expect(tab1.getAttribute('aria-selected')).toBe('true');
    expect(tab1.getAttribute('aria-controls')).toBe('my-panel-tab1');
    expect(tab1.tabIndex).toBe(0);
    expect(tab1.classList.contains('active')).toBe(true);
    expect(screen.getByTestId('icon-1')).toBeDefined();
    expect(screen.getByText('Tab One')).toBeDefined();
    expect(screen.getByText('First tab description')).toBeDefined();

    const tab2 = tabs[1];
    expect(tab2.id).toBe('my-card-tab-tab2');
    expect(tab2.getAttribute('aria-selected')).toBe('false');
    expect(tab2.getAttribute('aria-controls')).toBe('my-panel-tab2');
    expect(tab2.tabIndex).toBe(-1);
    expect(tab2.classList.contains('active')).toBe(false);

    const tab3 = tabs[2];
    expect(tab3.hasAttribute('disabled')).toBe(true);

    fireEvent.click(tab2);
    expect(onChange).toHaveBeenCalledWith('tab2');
  });

  it('navigates with arrow keys and wraps around, skipping disabled tabs and focusing active element', () => {
    const onChange = vi.fn();
    const { rerender } = render(
      <ScenarioCardTabs items={cardItems} activeTab="tab1" onChange={onChange} />
    );

    const tab1 = screen.getByRole('tab', { name: 'Tab One' });
    const tab2 = screen.getByRole('tab', { name: 'Tab Two' });

    // ArrowRight moves from tab1 to tab2 and focuses tab2
    fireEvent.keyDown(tab1, { key: 'ArrowRight' });
    expect(onChange).toHaveBeenCalledWith('tab2');
    expect(document.activeElement).toBe(tab2);

    // Rerender as activeTab="tab2"
    rerender(<ScenarioCardTabs items={cardItems} activeTab="tab2" onChange={onChange} />);

    // ArrowRight from tab2 skips disabled tab3 and wraps to tab1
    fireEvent.keyDown(tab2, { key: 'ArrowRight' });
    expect(onChange).toHaveBeenCalledWith('tab1');
    expect(document.activeElement).toBe(tab1);
    // Rerender as activeTab="tab1"
    rerender(<ScenarioCardTabs items={cardItems} activeTab="tab1" onChange={onChange} />);

    // ArrowLeft from tab1 wraps to tab2 (skipping disabled tab3)
    fireEvent.keyDown(tab1, { key: 'ArrowLeft' });
    expect(onChange).toHaveBeenCalledWith('tab2');
    expect(document.activeElement).toBe(tab2);
  });

  it('supports Home and End keys for jumping to first and last enabled tabs', () => {
    const onChange = vi.fn();
    render(<ScenarioCardTabs items={cardItems} activeTab="tab1" onChange={onChange} />);

    const tab1 = screen.getByRole('tab', { name: 'Tab One' });
    const tab2 = screen.getByRole('tab', { name: 'Tab Two' });

    fireEvent.keyDown(tab1, { key: 'End' });
    // tab3 is disabled, so End lands on tab2 and focuses it
    expect(onChange).toHaveBeenCalledWith('tab2');
    expect(document.activeElement).toBe(tab2);

    fireEvent.keyDown(tab1, { key: 'Home' });
    expect(onChange).toHaveBeenCalledWith('tab1');
    expect(document.activeElement).toBe(tab1);
  });

  it('does not render sliding track for card tabs, using in-place card transitions instead', () => {
    const { container, rerender } = render(
      <ScenarioCardTabs items={cardItems} activeTab="tab1" onChange={vi.fn()} animated={true} />
    );

    expect(container.querySelector('.settings-subtab-slider')).toBeNull();
    expect(container.querySelector('.settings-scenario-cards-slider')).toBeNull();
    const tablist = screen.getByRole('tablist');
    expect(tablist.classList.contains('is-animated')).toBe(true);

    rerender(
      <ScenarioCardTabs items={cardItems} activeTab="tab1" onChange={vi.fn()} animated={false} />
    );

    expect(tablist.classList.contains('is-animated')).toBe(false);
  });

  it('makes the first enabled tab focusable when activeTab does not match or first tab is disabled', () => {
    const itemsWithDisabledFirst = [
      { value: 'tab0', label: 'Disabled Tab 0', disabled: true },
      { value: 'tab1', label: 'Enabled Tab 1' },
      { value: 'tab2', label: 'Enabled Tab 2' },
    ];
    render(
      <ScenarioCardTabs
        items={itemsWithDisabledFirst}
        activeTab="non-existent"
        onChange={vi.fn()}
      />
    );

    const tab0 = screen.getByRole('tab', { name: 'Disabled Tab 0' });
    const tab1 = screen.getByRole('tab', { name: 'Enabled Tab 1' });
    const tab2 = screen.getByRole('tab', { name: 'Enabled Tab 2' });
    expect(tab0.getAttribute('tabindex')).toBe('-1');
    expect(tab1.getAttribute('tabindex')).toBe('0');
    expect(tab2.getAttribute('tabindex')).toBe('-1');
  });
});

describe('SecondaryTabs & SegmentedSubTabs', () => {
  const segmentedItems = [
    {
      value: 'sub1',
      label: 'Sub Tab 1',
      tooltip: 'Tooltip for sub1',
      icon: <span data-testid="seg-icon-1">*</span>,
    },
    {
      value: 'sub2',
      label: 'Sub Tab 2',
      tooltip: 'Tooltip for sub2',
      tooltipPos: 'bottom' as const,
    },
  ];

  it('renders segmented pill tabs with tooltips and nav styling', () => {
    const onChange = vi.fn();
    render(
      <SegmentedSubTabs
        ariaLabel="Sub Navigation"
        items={segmentedItems}
        activeTab="sub1"
        onChange={onChange}
        idPrefix="test-subtab"
      />
    );

    const nav = screen.getByRole('tablist', { name: 'Sub Navigation' });
    expect(nav.classList.contains('settings-subtab-nav')).toBe(true);

    const tab1 = screen.getByRole('tab', { name: 'Sub Tab 1' });
    expect(tab1.id).toBe('test-subtab-sub1');
    expect(tab1.classList.contains('settings-subtab-btn')).toBe(true);
    expect(tab1.classList.contains('active')).toBe(true);
    expect(tab1.getAttribute('data-tooltip')).toBe('Tooltip for sub1');
    expect(tab1.getAttribute('data-tooltip-pos')).toBe('top');

    const tab2 = screen.getByRole('tab', { name: 'Sub Tab 2' });
    expect(tab2.getAttribute('data-tooltip')).toBe('Tooltip for sub2');
    expect(tab2.getAttribute('data-tooltip-pos')).toBe('bottom');

    fireEvent.click(tab2);
    expect(onChange).toHaveBeenCalledWith('sub2');
  });

  it('supports unified SecondaryTabs component with variant prop', () => {
    const onChange = vi.fn();
    const { rerender } = render(
      <SecondaryTabs variant="card" items={segmentedItems} activeTab="sub1" onChange={onChange} />
    );

    expect(screen.getByRole('tablist').classList.contains('settings-scenario-cards')).toBe(true);

    rerender(
      <SecondaryTabs
        variant="segmented"
        items={segmentedItems}
        activeTab="sub1"
        onChange={onChange}
      />
    );

    expect(screen.getByRole('tablist').classList.contains('settings-subtab-nav')).toBe(true);
  });

  it('supports keyboard navigation and focus management on segmented subtabs', () => {
    const onChange = vi.fn();
    const { rerender } = render(
      <SegmentedSubTabs items={segmentedItems} activeTab="sub1" onChange={onChange} />
    );

    const tab1 = screen.getByRole('tab', { name: 'Sub Tab 1' });
    const tab2 = screen.getByRole('tab', { name: 'Sub Tab 2' });

    fireEvent.keyDown(tab1, { key: 'ArrowRight' });
    expect(onChange).toHaveBeenCalledWith('sub2');
    expect(document.activeElement).toBe(tab2);

    rerender(<SegmentedSubTabs items={segmentedItems} activeTab="sub2" onChange={onChange} />);

    fireEvent.keyDown(tab2, { key: 'ArrowLeft' });
    expect(onChange).toHaveBeenCalledWith('sub1');
    expect(document.activeElement).toBe(tab1);
  });

  it('omits sliding indicator for segmented tabs when animated is false', () => {
    const { container } = render(
      <SegmentedSubTabs
        items={segmentedItems}
        activeTab="sub1"
        onChange={vi.fn()}
        animated={false}
      />
    );

    expect(container.querySelector('.settings-subtab-slider')).toBeNull();
  });

  it('renders sliding indicator for segmented tabs and updates position when measured', () => {
    const origGetBoundingClientRect = HTMLElement.prototype.getBoundingClientRect;
    HTMLElement.prototype.getBoundingClientRect = function () {
      if (this.classList.contains('settings-subtab-nav')) {
        return {
          left: 10,
          top: 20,
          width: 300,
          height: 40,
          right: 310,
          bottom: 60,
          x: 10,
          y: 20,
          toJSON: () => {},
        } as DOMRect;
      }
      return {
        left: 20,
        top: 22,
        width: 80,
        height: 36,
        right: 100,
        bottom: 58,
        x: 20,
        y: 22,
        toJSON: () => {},
      } as DOMRect;
    };

    try {
      const { container } = render(
        <SegmentedSubTabs items={segmentedItems} activeTab="sub1" onChange={vi.fn()} />
      );

      const nav = container.querySelector('.settings-subtab-nav');
      expect(nav?.classList.contains('has-slider')).toBe(true);

      const slider = container.querySelector('.settings-subtab-slider') as HTMLElement;
      expect(slider).toBeDefined();
      expect(slider?.getAttribute('aria-hidden')).toBe('true');
      expect(slider.style.transform).toBe('translate3d(10px, 2px, 0)');
      expect(slider.style.width).toBe('80px');
      expect(slider.style.height).toBe('36px');
      expect(slider.style.opacity).toBe('1');
    } finally {
      HTMLElement.prototype.getBoundingClientRect = origGetBoundingClientRect;
    }
  });
});
