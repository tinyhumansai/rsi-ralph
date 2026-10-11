"""Tabu Search graph coloring algorithm."""

import time
import random


def color(n, edges):
    start_time = time.time()
    
    # Build adjacency lists
    neighbours = [set() for _ in range(n)]
    for a, b in edges:
        neighbours[a].add(b)
        neighbours[b].add(a)
    
    # DSATUR for initial coloring
    colors = dsatur(n, neighbours)
    initial_colors = max(colors) + 1
    print(f"DIAG DSATUR initial: {initial_colors} colors")
    
    # Try to reduce colors using tabu search
    best_colors = colors[:]
    best_k = initial_colors
    
    for target_k in range(initial_colors - 1, 5, -1):
        if time.time() - start_time > 18.0:
            break
        
        result = tabu_search(n, neighbours, target_k, time.time() - start_time)
        if result is not None:
            best_colors = result
            best_k = target_k
            print(f"DIAG Found {target_k}-coloring")
        else:
            print(f"DIAG Failed to find {target_k}-coloring, stopping")
            break
    
    color_counts = [best_colors.count(i) for i in range(best_k)]
    elapsed = time.time() - start_time
    print(f"DIAG Final: {best_k} colors, sizes {color_counts} in {elapsed:.2f}s")
    
    return best_colors


def dsatur(n, neighbours):
    colors = [-1] * n
    uncolored = set(range(n))
    
    # Start with highest degree vertex
    max_degree_vertex = max(range(n), key=lambda v: len(neighbours[v]))
    colors[max_degree_vertex] = 0
    uncolored.remove(max_degree_vertex)
    
    while uncolored:
        best_vertex = None
        best_saturation = -1
        best_degree = -1
        
        for v in uncolored:
            neighbor_colors = {colors[u] for u in neighbours[v] if colors[u] != -1}
            saturation = len(neighbor_colors)
            uncolored_degree = sum(1 for u in neighbours[v] if colors[u] == -1)
            
            if (saturation > best_saturation or 
                (saturation == best_saturation and uncolored_degree > best_degree)):
                best_vertex = v
                best_saturation = saturation
                best_degree = uncolored_degree
        
        v = best_vertex
        used_colors = {colors[u] for u in neighbours[v] if colors[u] != -1}
        c = 0
        while c in used_colors:
            c += 1
        colors[v] = c
        uncolored.remove(v)
    
    return colors


def tabu_search(n, neighbours, k, elapsed_time):
    """Try to find a valid k-coloring using tabu search."""
    import os
    seed = int(os.environ.get('TINYRSI_SEED', '0'))
    rng = random.Random(seed + k)  # Deterministic per k
    
    # Initialize with random k-coloring
    colors = [rng.randint(0, k-1) for _ in range(n)]
    
    # Compute initial conflicts
    conflicts = compute_conflicts(n, neighbours, colors)
    
    if sum(conflicts) == 0:
        return colors
    
    tabu_list = {}  # (vertex, color) -> iteration when it becomes non-tabu
    tabu_tenure = 10
    max_iterations = 10000
    time_limit = 18.0
    
    best_conflicts = sum(conflicts)
    best_colors = colors[:]
    iterations_without_improvement = 0
    
    for iteration in range(max_iterations):
        if time.time() - time.time() + elapsed_time > time_limit:
            break
        
        if sum(conflicts) == 0:
            return colors
        
        # Find best move (vertex, new_color)
        best_move = None
        best_delta = float('inf')
        
        # Focus on vertices with conflicts
        conflict_vertices = [v for v in range(n) if conflicts[v] > 0]
        if not conflict_vertices:
            conflict_vertices = list(range(n))
        
        # Sample vertices to keep it fast
        sample_size = min(30, len(conflict_vertices))
        sampled_vertices = rng.sample(conflict_vertices, sample_size)
        
        for v in sampled_vertices:
            current_color = colors[v]
            for new_color in range(k):
                if new_color == current_color:
                    continue
                
                # Check if move is tabu
                if (v, new_color) in tabu_list and tabu_list[(v, new_color)] > iteration:
                    # Aspiration criterion: allow if it improves best solution
                    delta = compute_move_delta(v, new_color, colors, neighbours, conflicts)
                    if sum(conflicts) + delta >= best_conflicts:
                        continue
                
                delta = compute_move_delta(v, new_color, colors, neighbours, conflicts)
                if delta < best_delta:
                    best_delta = delta
                    best_move = (v, new_color)
        
        if best_move is None:
            break
        
        # Apply best move
        v, new_color = best_move
        old_color = colors[v]
        apply_move(v, new_color, colors, neighbours, conflicts)
        
        # Update tabu list
        tabu_list[(v, old_color)] = iteration + tabu_tenure
        
        # Track best solution
        current_conflicts = sum(conflicts)
        if current_conflicts < best_conflicts:
            best_conflicts = current_conflicts
            best_colors = colors[:]
            iterations_without_improvement = 0
        else:
            iterations_without_improvement += 1
        
        # Early termination if stuck
        if iterations_without_improvement > 500:
            break
    
    if sum(compute_conflicts(n, neighbours, best_colors)) == 0:
        return best_colors
    return None


def compute_conflicts(n, neighbours, colors):
    """Return list of conflict counts per vertex."""
    conflicts = [0] * n
    for v in range(n):
        for u in neighbours[v]:
            if colors[v] == colors[u]:
                conflicts[v] += 1
    return conflicts


def compute_move_delta(v, new_color, colors, neighbours, conflicts):
    """Compute change in total conflicts if v is recolored to new_color."""
    delta = 0
    old_color = colors[v]
    
    for u in neighbours[v]:
        # Remove old conflicts
        if colors[u] == old_color:
            delta -= 2  # Both v and u lose a conflict
        # Add new conflicts
        if colors[u] == new_color:
            delta += 2  # Both v and u gain a conflict
    
    return delta


def apply_move(v, new_color, colors, neighbours, conflicts):
    """Apply move and update conflicts array."""
    old_color = colors[v]
    
    for u in neighbours[v]:
        if colors[u] == old_color:
            conflicts[u] -= 1
            conflicts[v] -= 1
        if colors[u] == new_color:
            conflicts[u] += 1
            conflicts[v] += 1
    
    colors[v] = new_color
