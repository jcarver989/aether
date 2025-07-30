# Aether Scrolling Implementation Improvement Plan

## Overview

Based on analysis of gitui's efficient scrolling implementation, this plan outlines improvements to Aether's virtual scrolling system. The key insight is that **simplicity and laziness win** - gitui achieves smooth performance by avoiding complex calculations and only processing visible content.

## Current Issues with Aether's Implementation

1. **Over-engineering**: Complex cumulative height calculations for all items
2. **Eager calculation**: Computing heights for all items upfront
3. **Cache invalidation**: Frequent full cache rebuilds
4. **Pixel-perfect positioning**: Unnecessary complexity for terminal UIs
5. **Mutable state requirements**: Need for `&mut self` in many operations

## Key Principles from gitui

1. **Line-based scrolling**: Treat content as uniform lines, not variable heights
2. **Lazy evaluation**: Only process what's visible
3. **Interior mutability**: Use `Cell<T>` for cleaner APIs
4. **Simple math**: Basic index calculations instead of complex algorithms
5. **Chunk-based loading**: Load data in fixed-size chunks as needed

## Proposed Architecture

### 1. Simplified Data Structure

```rust
pub struct VirtualScroll<T: VirtualScrollItem> {
    // Core data
    items: Vec<T>,
    
    // Scroll state (using Cell for interior mutability)
    scroll_top: Cell<usize>,      // Top visible line
    viewport_height: Cell<usize>,  // Viewport height in lines
    
    // Selection
    selection: Cell<usize>,        // Selected item index
    
    // Optional optimization
    line_cache: RefCell<HashMap<usize, Vec<String>>>, // Cached wrapped lines
    cache_width: Cell<u16>,        // Width used for cache
}
```

### 2. Simplified Item Trait

```rust
pub trait VirtualScrollItem {
    /// Get lines for this item when rendered at the given width
    /// Returns wrapped/formatted lines
    fn render_lines(&self, width: u16) -> Vec<String>;
    
    /// Estimated height in lines (for rough scrollbar positioning)
    fn estimated_height(&self) -> usize {
        1 // Default to 1 line
    }
}
```

### 3. Core Operations

#### Scrolling
```rust
impl<T: VirtualScrollItem> VirtualScroll<T> {
    /// Ensure the selected item is visible
    fn ensure_visible(&self, index: usize) {
        let viewport_height = self.viewport_height.get();
        let current_top = self.scroll_top.get();
        
        // Use gitui's simple algorithm
        let new_top = if current_top + viewport_height <= index {
            index.saturating_sub(viewport_height) + 1
        } else if current_top > index {
            index
        } else {
            current_top
        };
        
        self.scroll_top.set(new_top);
    }
    
    /// Scroll by a certain amount
    fn scroll(&self, direction: ScrollDirection) {
        let current = self.scroll_top.get();
        let viewport = self.viewport_height.get();
        let max_scroll = self.items.len().saturating_sub(viewport);
        
        let new_top = match direction {
            ScrollDirection::Up => current.saturating_sub(1),
            ScrollDirection::Down => current.saturating_add(1).min(max_scroll),
            ScrollDirection::PageUp => current.saturating_sub(viewport),
            ScrollDirection::PageDown => (current + viewport).min(max_scroll),
            ScrollDirection::Home => 0,
            ScrollDirection::End => max_scroll,
        };
        
        self.scroll_top.set(new_top);
    }
}
```

#### Rendering
```rust
impl<T: VirtualScrollItem> VirtualScroll<T> {
    /// Get items that should be rendered
    fn get_visible_items(&self, width: u16) -> Vec<(usize, Vec<String>)> {
        let start = self.scroll_top.get();
        let viewport_height = self.viewport_height.get();
        
        let mut result = Vec::new();
        let mut lines_count = 0;
        
        // Only process items that might be visible
        for (idx, item) in self.items.iter().enumerate().skip(start) {
            let lines = self.get_cached_lines(idx, item, width);
            result.push((idx, lines.clone()));
            
            lines_count += lines.len();
            if lines_count >= viewport_height {
                break; // We have enough to fill the viewport
            }
        }
        
        result
    }
    
    /// Get cached lines or render new ones
    fn get_cached_lines(&self, idx: usize, item: &T, width: u16) -> Vec<String> {
        let mut cache = self.line_cache.borrow_mut();
        
        // Invalidate cache if width changed
        if self.cache_width.get() != width {
            cache.clear();
            self.cache_width.set(width);
        }
        
        cache.entry(idx)
            .or_insert_with(|| item.render_lines(width))
            .clone()
    }
}
```

### 4. Optimizations

#### Chunk-Based Loading (for large datasets)
```rust
const CHUNK_SIZE: usize = 1000;

pub struct ChunkedVirtualScroll<T> {
    // Only load chunks of data as needed
    loaded_chunks: HashMap<usize, Vec<T>>,
    total_items: usize,
    // ... other fields
}

impl<T> ChunkedVirtualScroll<T> {
    fn ensure_chunk_loaded(&mut self, index: usize) {
        let chunk_idx = index / CHUNK_SIZE;
        if !self.loaded_chunks.contains_key(&chunk_idx) {
            // Load chunk from source
            let chunk = self.load_chunk(chunk_idx);
            self.loaded_chunks.insert(chunk_idx, chunk);
        }
    }
}
```

#### Smart Cache Management
```rust
impl<T: VirtualScrollItem> VirtualScroll<T> {
    /// Clean up cache entries far from viewport
    fn cleanup_cache(&self) {
        let current_top = self.scroll_top.get();
        let viewport = self.viewport_height.get();
        let buffer = viewport * 2; // Keep 2x viewport in cache
        
        let mut cache = self.line_cache.borrow_mut();
        cache.retain(|&idx, _| {
            idx >= current_top.saturating_sub(buffer) 
                && idx <= current_top + viewport + buffer
        });
    }
}
```

## Implementation Plan

### Phase 1: Core Refactoring (Priority: High)
1. Replace cumulative height tracking with line-based approach
2. Convert to interior mutability pattern using `Cell`/`RefCell`
3. Implement lazy line rendering for visible items only
4. Add simple line caching for wrapped text

### Phase 2: Performance Optimizations (Priority: Medium)
1. Implement smart cache management
2. Add viewport buffer for smooth scrolling
3. Optimize for streaming content (chat messages)
4. Add benchmarks to measure improvements

### Phase 3: Advanced Features (Priority: Low)
1. Implement chunk-based loading for very large datasets
2. Add smooth scrolling animations (if supported by terminal)
3. Implement variable-height item support (only if needed)
4. Add jump-to navigation (e.g., jump to next tool call)

## Benefits

1. **Better Performance**: Only process visible content
2. **Lower Memory Usage**: No need to cache heights for all items
3. **Simpler Code**: Remove complex cumulative height calculations
4. **Better UX**: Smoother scrolling, especially with large datasets
5. **Cleaner API**: Interior mutability removes need for `&mut self`

## Migration Strategy

1. Create new `SimpleVirtualScroll` component alongside existing one
2. Implement core functionality with new architecture
3. Add compatibility layer for existing `VirtualScrollItem` trait
4. Gradually migrate components to use new implementation
5. Remove old implementation once all components migrated

## Success Metrics

- [ ] Scrolling remains smooth with 10,000+ chat messages
- [ ] Memory usage doesn't grow linearly with message count
- [ ] No noticeable lag when appending streaming content
- [ ] Simplified code with fewer lines than current implementation
- [ ] All existing scrolling features maintained

## Next Steps

1. Create a proof-of-concept implementation
2. Benchmark against current implementation
3. Test with real-world chat scenarios
4. Gather feedback from users
5. Implement full solution based on learnings