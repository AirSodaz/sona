import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { createRef } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { SearchInput } from '../SearchInput';

describe('SearchInput', () => {
  it('updates the controlled value and forwards the native change event once', () => {
    const onValueChange = vi.fn();
    const onChange = vi.fn();
    const inputRef = createRef<HTMLInputElement>();

    render(
      <SearchInput
        ref={inputRef}
        id="library-search"
        type="search"
        required
        placeholder="Search library"
        aria-label="Search library"
        data-source="library"
        value=""
        onValueChange={onValueChange}
        onChange={onChange}
        suffix={<span>Advanced filters</span>}
      />
    );

    const input = screen.getByRole('searchbox', { name: 'Search library' });
    expect(inputRef.current).toBe(input);
    expect(input.getAttribute('id')).toBe('library-search');
    expect(input.getAttribute('placeholder')).toBe('Search library');
    expect((input as HTMLInputElement).required).toBe(true);
    expect(input.getAttribute('data-source')).toBe('library');
    expect(screen.getByText('Advanced filters')).toBeTruthy();

    fireEvent.change(input, { target: { value: 'film' } });

    expect(onValueChange).toHaveBeenCalledTimes(1);
    expect(onValueChange).toHaveBeenCalledWith('film');
    expect(onChange).toHaveBeenCalledTimes(1);
  });

  it('uses onValueChange as the clear fallback and focuses the input', async () => {
    const onValueChange = vi.fn();
    render(<SearchInput aria-label="Search" value="query" onValueChange={onValueChange} />);

    const input = screen.getByRole('textbox', { name: 'Search' });
    fireEvent.click(screen.getByRole('button', { name: 'Clear search' }));

    expect(onValueChange).toHaveBeenCalledTimes(1);
    expect(onValueChange).toHaveBeenCalledWith('');
    await waitFor(() => expect(document.activeElement).toBe(input));
  });

  it('calls a custom clear handler without also changing the value', () => {
    const onValueChange = vi.fn();
    const onClear = vi.fn();
    render(
      <SearchInput
        aria-label="Search"
        clearLabel="Clear query"
        value="query"
        onValueChange={onValueChange}
        onClear={onClear}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: 'Clear query' }));

    expect(onClear).toHaveBeenCalledTimes(1);
    expect(onValueChange).not.toHaveBeenCalled();
  });

  it('only shows the clear button while there is a value', () => {
    const props = { 'aria-label': 'Search', onValueChange: vi.fn() };
    const { rerender } = render(<SearchInput {...props} value="" />);
    expect(screen.queryByRole('button', { name: 'Clear search' })).toBeNull();

    rerender(<SearchInput {...props} value="query" />);
    expect(screen.getByRole('button', { name: 'Clear search' })).toBeTruthy();

    rerender(<SearchInput {...props} value="" />);
    expect(screen.queryByRole('button', { name: 'Clear search' })).toBeNull();
  });

  it('forwards callback refs, input classes and keyboard events without intercepting them', () => {
    const ref = vi.fn();
    const onKeyDown = vi.fn();
    const onFocus = vi.fn();
    const { container } = render(
      <SearchInput
        ref={ref}
        value="query"
        onValueChange={vi.fn()}
        aria-label="Search"
        wrapperClassName="custom-wrapper"
        inputClassName="custom-input"
        onKeyDown={onKeyDown}
        onFocus={onFocus}
      />
    );
    const input = screen.getByRole('textbox', { name: 'Search' });
    expect(ref).toHaveBeenCalledWith(input);
    expect(input.classList.contains('custom-input')).toBe(true);
    expect(container.firstElementChild?.classList.contains('custom-wrapper')).toBe(true);
    fireEvent.focus(input);
    expect(onFocus).toHaveBeenCalledTimes(1);
    for (const key of ['Escape', 'Enter', 'ArrowDown', 'ArrowUp']) {
      expect(fireEvent.keyDown(input, { key })).toBe(true);
    }
    expect(onKeyDown).toHaveBeenCalledTimes(4);
  });

  it('does not clear disabled or read-only input values', () => {
    const disabledClear = vi.fn();
    const readOnlyClear = vi.fn();
    const { rerender } = render(
      <SearchInput
        aria-label="Disabled search"
        value="query"
        disabled
        onValueChange={vi.fn()}
        onClear={disabledClear}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: 'Clear search' }));
    expect(disabledClear).not.toHaveBeenCalled();

    rerender(
      <SearchInput
        aria-label="Read-only search"
        value="query"
        readOnly
        onValueChange={vi.fn()}
        onClear={readOnlyClear}
      />
    );

    expect(screen.queryByRole('button', { name: 'Clear search' })).toBeNull();
    expect(readOnlyClear).not.toHaveBeenCalled();
  });

  it('focuses input when clicking the search icon or wrapper', () => {
    render(<SearchInput aria-label="Search" value="" onValueChange={vi.fn()} />);
    const input = screen.getByRole('textbox', { name: 'Search' });
    const wrapper = screen.getByTestId('search-input');
    const icon = wrapper.querySelector('.sona-search-input__icon');
    expect(icon).toBeTruthy();

    fireEvent.click(icon!);
    expect(document.activeElement).toBe(input);

    input.blur();
    expect(document.activeElement).not.toBe(input);

    fireEvent.click(wrapper);
    expect(document.activeElement).toBe(input);
  });

  it('hides the clear button when showClearButton is false', () => {
    render(
      <SearchInput
        aria-label="Search"
        value="query"
        onValueChange={vi.fn()}
        showClearButton={false}
      />
    );
    expect(screen.queryByRole('button', { name: 'Clear search' })).toBeNull();
  });

  it('stops click propagation when clicking the clear button', () => {
    const parentClick = vi.fn();
    render(
      <div onClick={parentClick}>
        <SearchInput aria-label="Search" value="query" onValueChange={vi.fn()} />
      </div>
    );
    const clearButton = screen.getByRole('button', { name: 'Clear search' });
    fireEvent.click(clearButton);
    expect(parentClick).not.toHaveBeenCalled();
  });
});
