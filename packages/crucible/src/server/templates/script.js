// State
let currentRunId = null;
let evalResults = []; // All evals (started, running, completed) from store
let currentEval = null;
let currentTraces = []; // Cache for currently displayed traces
let expandedTraces = new Set();
let allExpanded = false;
let searchTerm = '';
let currentTab = 'assertions'; // 'assertions', 'traces', 'code_changes'
let eventSource = null;
let isLive = false;

// Initialize
document.addEventListener('DOMContentLoaded', async () => {
    try {
        // Fetch list of runs
        const runsResponse = await fetch('/api/runs');
        const runs = await runsResponse.json();

        if (runs.length === 0) {
            document.getElementById('empty-state').innerHTML =
                '<p style="color: var(--text-muted);">No evaluation runs found</p>';
            return;
        }

        // Use the most recent run (first in the list)
        currentRunId = runs[0];

        // Connect to SSE FIRST, before loading results, so we don't miss any events
        connectSSE();

        // Fetch all eval results for this run (includes started, running, completed)
        const resultsResponse = await fetch(`/api/runs/${currentRunId}`);
        evalResults = await resultsResponse.json();

        renderSummary();
        renderEvalList();
        setupEventListeners();
    } catch (error) {
        console.error('Failed to load report data:', error);
        document.getElementById('empty-state').innerHTML =
            `<p style="color: var(--failure);">Failed to load report data: ${error.message}</p>`;
    }
});

// Connect to SSE for real-time updates
function connectSSE() {
    try {
        // Use run-specific endpoint to only receive events for this run
        const endpoint = currentRunId ? `/api/runs/${currentRunId}/events` : '/api/events';
        eventSource = new EventSource(endpoint);

        eventSource.onopen = () => {
            isLive = true;
            updateConnectionStatus();
            console.log('SSE connected');
        };

        eventSource.onerror = (error) => {
            isLive = false;
            updateConnectionStatus();
            console.error('SSE connection error. ReadyState:', eventSource.readyState);

            // ReadyState: 0 = CONNECTING, 1 = OPEN, 2 = CLOSED
            if (eventSource.readyState === EventSource.CLOSED) {
                console.log('SSE connection permanently closed by server');
            } else if (eventSource.readyState === EventSource.CONNECTING) {
                console.log('SSE reconnecting...');
            }
        };

        eventSource.onmessage = (e) => {
            const event = JSON.parse(e.data);
            console.log('SSE event received:', event.type);

            switch (event.type) {
                case 'eval_started':
                    handleEvalStarted(event);
                    break;
                case 'eval_completed':
                    handleEvalCompleted(event);
                    break;
                case 'run_completed':
                    handleRunCompleted(event);
                    break;
                case 'trace_event':
                    handleTraceEvent(event);
                    break;
            }
        };
    } catch (error) {
        console.error('Error connecting to SSE:', error);
        isLive = false;
        updateConnectionStatus();
    }
}

// SSE Event Handlers
function handleEvalStarted(event) {
    console.log('Eval started:', event.name);

    // Only track evals for the current run
    if (event.run_id !== currentRunId) return;

    // Check if we already have this eval in our results
    const existingIndex = evalResults.findIndex(e => e.id === event.eval_id);
    if (existingIndex < 0) {
        // Add new started eval
        evalResults.push({
            status: 'started',
            id: event.eval_id,
            eval_name: event.name
        });
        renderEvalList();
    }
}

function handleEvalCompleted(event) {
    console.log('Eval completed:', event.name);

    // Only process evals for the current run
    if (event.run_id !== currentRunId) return;

    // Update or add the completed eval result
    const evalIndex = evalResults.findIndex(e => e.id === event.eval_id);
    if (evalIndex >= 0) {
        evalResults[evalIndex] = event.report;
    } else {
        evalResults.push(event.report);
    }

    // Re-render summary and list
    renderSummary();
    renderEvalList();

    // If this is the currently selected eval, update the details
    if (currentEval === event.eval_id) {
        renderEvalDetails();
    }
}

function handleRunCompleted(event) {
    console.log('Run completed:', event.run_id);

    if (event.run_id === currentRunId) {
        console.log('Current run completed');
        // Could optionally reload full results here
    }
}

function handleTraceEvent(event) {
    // If viewing this eval's traces, fetch and re-render
    if (currentEval === event.eval_id && currentTab === 'traces') {
        fetchAndRenderTraces(event.eval_id);
    }
}

// Update connection status indicator
function updateConnectionStatus() {
    const existingStatus = document.getElementById('connection-status');
    if (existingStatus) {
        existingStatus.remove();
    }

    if (isLive) {
        const statusBadge = document.createElement('div');
        statusBadge.id = 'connection-status';
        statusBadge.className = 'connection-badge live';
        statusBadge.innerHTML = '<span class="live-dot"></span> Live';
        document.querySelector('.header').appendChild(statusBadge);
    }
}

// Event Listeners
function setupEventListeners() {
    const searchInput = document.getElementById('search-input');
    searchInput.addEventListener('input', (e) => {
        searchTerm = e.target.value.toLowerCase();
        if (currentTab === 'traces' && currentTraces.length > 0) {
            renderTracesWithData();
        }
    });
}

// Render Summary Stats
function renderSummary() {
    // Only count completed evals in summary
    const completedEvals = evalResults.filter(e => e.status === 'completed');
    const totalEvals = completedEvals.length;
    const passedEvals = completedEvals.filter(e => e.passed).length;
    const failedEvals = totalEvals - passedEvals;

    const totalAssertions = completedEvals.reduce((sum, e) => sum + (e.assertions?.length || 0), 0);
    const passedAssertions = completedEvals.reduce(
        (sum, e) => sum + (e.assertions?.filter(a => a.passed).length || 0),
        0
    );
    const failedAssertions = totalAssertions - passedAssertions;

    const statsHtml = `
        <div class="stat">
            <div class="stat-label">Evals</div>
            <div class="stat-value">
                <span class="success">${passedEvals}</span> /
                <span class="failure">${failedEvals}</span> /
                ${totalEvals}
            </div>
        </div>
        <div class="stat">
            <div class="stat-label">Assertions</div>
            <div class="stat-value">
                <span class="success">${passedAssertions}</span> /
                <span class="failure">${failedAssertions}</span> /
                ${totalAssertions}
            </div>
        </div>
        <div class="stat">
            <div class="stat-label">Success Rate</div>
            <div class="stat-value ${passedEvals === totalEvals ? 'success' : 'failure'}">
                ${totalEvals > 0 ? Math.round((passedEvals / totalEvals) * 100) : 0}%
            </div>
        </div>
    `;
    document.getElementById('summary-stats').innerHTML = statsHtml;
}

// Render Eval List
function renderEvalList() {
    const evalListHtml = evalResults.map(eval => {
        if (eval.status === 'completed') {
            // Completed eval
            const status = eval.passed ? '✓' : '✗';
            const statusColor = eval.passed ? 'success' : 'failure';
            const assertionsPassed = eval.assertions?.filter(a => a.passed).length || 0;
            const totalAssertions = eval.assertions?.length || 0;

            return `
                <div class="eval-item" data-eval="${eval.id}" onclick="selectEval('${eval.id}')">
                    <div class="eval-status" style="color: var(--${statusColor})">${status}</div>
                    <div style="flex: 1; min-width: 0;">
                        <div class="eval-name">${escapeHtml(eval.eval_name)}</div>
                        <div class="eval-meta">${assertionsPassed}/${totalAssertions} assertions passed</div>
                    </div>
                </div>
            `;
        } else {
            // Started or running eval
            return `
                <div class="eval-item running" data-eval="${eval.id}">
                    <div class="eval-status" style="color: var(--text-muted)">⟳</div>
                    <div style="flex: 1; min-width: 0;">
                        <div class="eval-name">${escapeHtml(eval.eval_name)}</div>
                        <div class="eval-meta" style="color: var(--text-muted)">${eval.status === 'running' ? 'Running' : 'Started'}...</div>
                    </div>
                </div>
            `;
        }
    }).join('');

    document.getElementById('eval-list').innerHTML = evalListHtml;
}

// Select Eval
function selectEval(evalId) {
    currentEval = evalId;
    expandedTraces.clear();
    allExpanded = false;
    currentTab = 'assertions'; // Reset to default tab

    // Update active state in sidebar
    document.querySelectorAll('.eval-item').forEach(item => {
        item.classList.toggle('active', item.dataset.eval === evalId);
    });

    // Show eval details
    document.getElementById('empty-state').style.display = 'none';
    document.getElementById('eval-details').style.display = 'block';

    renderEvalDetails();
}

// Switch Tab
function switchTab(tabName) {
    currentTab = tabName;
    renderEvalDetails();
}

// Render Eval Details
function renderEvalDetails() {
    const eval = evalResults.find(e => e.id === currentEval);
    if (!eval) return;

    // Handle non-completed evals
    if (eval.status !== 'completed') {
        const statusText = eval.status === 'running' ? 'Running' : 'Started';
        document.getElementById('eval-details').innerHTML = `
            <div class="eval-header">
                <h2 class="eval-title">${escapeHtml(eval.eval_name)}</h2>
                <div class="eval-subtitle">
                    <span class="status-badge" style="background: var(--text-muted)">⟳ ${statusText}...</span>
                </div>
            </div>
            <div style="padding: 2rem; text-align: center; color: var(--text-muted);">
                <p>This evaluation is currently ${statusText.toLowerCase()}. Results will appear here when complete.</p>
            </div>
        `;
        return;
    }

    const statusBadge = eval.passed
        ? '<span class="status-badge success">✓ Passed</span>'
        : '<span class="status-badge failure">✗ Failed</span>';

    // Check if code changes are available
    const hasCodeChanges = eval.agent_diff || eval.gold_diff;

    // Build tabs
    const tabsHtml = `
        <div class="tabs">
            <button class="tab ${currentTab === 'assertions' ? 'active' : ''}" onclick="switchTab('assertions')">
                Assertions
            </button>
            <button class="tab ${currentTab === 'traces' ? 'active' : ''}" onclick="switchTab('traces')">
                Traces
            </button>
            ${hasCodeChanges ? `
            <button class="tab ${currentTab === 'code_changes' ? 'active' : ''}" onclick="switchTab('code_changes')">
                Code Changes
            </button>
            ` : ''}
        </div>
    `;

    let contentHtml = '';

    if (currentTab === 'assertions') {
        const assertionsHtml = eval.assertions.map(assertion => {
            const status = assertion.passed ? 'success' : 'failure';
            return `
                <div class="assertion ${status}">
                    <div class="assertion-type">${escapeHtml(assertion.assertion_type)}</div>
                    <div class="assertion-message">${escapeHtml(assertion.message)}</div>
                </div>
            `;
        }).join('');

        contentHtml = `
            <div class="assertions-section">
                <h3 class="section-title">Assertions (${eval.assertions.filter(a => a.passed).length}/${eval.assertions.length})</h3>
                ${assertionsHtml}
            </div>
        `;
    } else if (currentTab === 'traces') {
        contentHtml = `
            <div class="traces-section">
                <div class="traces-header">
                    <h3 class="section-title">Traces</h3>
                    <button class="expand-all-btn" onclick="toggleExpandAll()">
                        ${allExpanded ? 'Collapse All' : 'Expand All'}
                    </button>
                </div>
                <div class="timeline" id="timeline">
                    <!-- Populated by renderTraces() -->
                </div>
            </div>
        `;
    } else if (currentTab === 'code_changes' && hasCodeChanges) {
        contentHtml = renderCodeChanges(eval);
    }

    const detailsHtml = `
        <div class="eval-header">
            <h2 class="eval-title">${escapeHtml(eval.eval_name)}</h2>
            <div class="eval-subtitle">${statusBadge}</div>
        </div>
        ${tabsHtml}
        ${contentHtml}
    `;

    document.getElementById('eval-details').innerHTML = detailsHtml;

    if (currentTab === 'traces') {
        renderTraces();
    } else if (currentTab === 'code_changes' && hasCodeChanges) {
        setupDiffScrollSync();
    }
}

// Fetch and Render Traces
async function fetchAndRenderTraces(evalId) {
    try {
        const response = await fetch(`/api/runs/${currentRunId}/evals/${evalId}/traces`);
        currentTraces = await response.json();
        renderTracesWithData();
    } catch (error) {
        console.error('Failed to fetch traces:', error);
        document.getElementById('timeline').innerHTML =
            '<p style="color: var(--failure); text-align: center; padding: 2rem;">Failed to load traces</p>';
    }
}

// Render Traces
async function renderTraces() {
    await fetchAndRenderTraces(currentEval);
}

function renderTracesWithData() {
    // Filter traces by search term
    const filteredTraces = currentTraces.filter(trace => {
        if (!searchTerm) return true;
        const message = trace.fields?.message || '';
        const searchableText = `${message} ${trace.level} ${trace.target}`.toLowerCase();
        return searchableText.includes(searchTerm);
    });

    if (filteredTraces.length === 0) {
        document.getElementById('timeline').innerHTML =
            '<p style="color: var(--text-muted); text-align: center; padding: 2rem;">No traces found' +
            (searchTerm ? ' matching your search' : '') + '</p>';
        return;
    }

    // Calculate relative timestamps
    const firstTimestamp = filteredTraces[0]?.timestamp;

    const tracesHtml = filteredTraces.map((trace, index) => {
        const traceId = `trace-${index}`;
        const isExpanded = expandedTraces.has(traceId);
        const level = trace.level.toLowerCase();
        const relativeTime = calculateRelativeTime(firstTimestamp, trace.timestamp);
        const message = trace.fields?.message || '';

        // Check if there are extra details to show (fields, span, or target)
        const hasDetails = trace.fields || trace.span || trace.target;

        return `
            <div class="trace-event">
                <div class="trace-marker ${level}"></div>
                <div class="trace-content">
                    <div class="trace-header" onclick="${hasDetails ? `toggleTrace('${traceId}')` : ''}">
                        <span class="trace-level ${level}">${escapeHtml(trace.level)}</span>
                        <span class="trace-timestamp">${relativeTime}</span>
                        <span class="trace-message">${escapeHtml(message)}</span>
                        ${hasDetails ? `<span class="trace-expand-icon ${isExpanded ? 'expanded' : ''}">▶</span>` : ''}
                    </div>
                    ${isExpanded && hasDetails ? renderTraceDetails(trace) : ''}
                </div>
            </div>
        `;
    }).join('');

    document.getElementById('timeline').innerHTML = tracesHtml;
}

// Render Trace Details
function renderTraceDetails(trace) {
    const details = {
        target: trace.target,
        fields: trace.fields,
        span: trace.span,
        spans: trace.spans
    };

    return `
        <div class="trace-details">
            <pre>${escapeHtml(JSON.stringify(details, null, 2))}</pre>
        </div>
    `;
}

// Toggle Trace Expansion
function toggleTrace(traceId) {
    if (expandedTraces.has(traceId)) {
        expandedTraces.delete(traceId);
    } else {
        expandedTraces.add(traceId);
    }
    renderTracesWithData();
}

// Toggle Expand All
function toggleExpandAll() {
    allExpanded = !allExpanded;
    expandedTraces.clear();

    if (allExpanded) {
        currentTraces.forEach((_, index) => {
            expandedTraces.add(`trace-${index}`);
        });
    }

    renderTracesWithData();
}

// Calculate Relative Time
function calculateRelativeTime(startTime, currentTime) {
    if (!startTime || !currentTime) return '0.0s';

    const start = new Date(startTime).getTime();
    const current = new Date(currentTime).getTime();
    const diffMs = current - start;

    if (diffMs < 0) return '0.0s';
    if (diffMs < 1000) return `+${diffMs}ms`;

    const diffSec = (diffMs / 1000).toFixed(1);
    return `+${diffSec}s`;
}

// Escape HTML
function escapeHtml(text) {
    if (typeof text !== 'string') return text;
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

// Render Code Changes (Side-by-Side Diff Viewer)
function renderCodeChanges(eval) {
    const diffStats = eval.diff_stats || { files_changed: 0, lines_added: 0, lines_removed: 0 };

    const statsHtml = `
        <div class="diff-stats">
            <div class="diff-stat">
                <span class="diff-stat-label">Files Changed:</span>
                <span class="diff-stat-value">${diffStats.files_changed}</span>
            </div>
            <div class="diff-stat">
                <span class="diff-stat-label">Lines Added:</span>
                <span class="diff-stat-value success">+${diffStats.lines_added}</span>
            </div>
            <div class="diff-stat">
                <span class="diff-stat-label">Lines Removed:</span>
                <span class="diff-stat-value failure">-${diffStats.lines_removed}</span>
            </div>
        </div>
    `;

    const agentDiffHtml = eval.agent_diff ? formatDiff(eval.agent_diff) : '<p class="no-diff">No agent changes</p>';
    const goldDiffHtml = eval.gold_diff ? formatDiff(eval.gold_diff) : '<p class="no-diff">No gold diff available</p>';

    return `
        <div class="code-changes-section">
            <h3 class="section-title">Code Changes Comparison</h3>
            ${statsHtml}
            <div class="diff-viewer">
                <div class="diff-pane" id="agent-diff-pane">
                    <div class="diff-pane-header">Agent Changes</div>
                    <div class="diff-pane-content" id="agent-diff-content">
                        ${agentDiffHtml}
                    </div>
                </div>
                <div class="diff-pane" id="gold-diff-pane">
                    <div class="diff-pane-header">Human Solution (Gold)</div>
                    <div class="diff-pane-content" id="gold-diff-content">
                        ${goldDiffHtml}
                    </div>
                </div>
            </div>
        </div>
    `;
}

// Format Diff with Syntax Highlighting
function formatDiff(diffText) {
    const lines = diffText.split('\n');
    const formattedLines = lines.map(line => {
        let className = 'diff-line-context';
        if (line.startsWith('+++') || line.startsWith('---')) {
            className = 'diff-line-file';
        } else if (line.startsWith('diff --git')) {
            className = 'diff-line-header';
        } else if (line.startsWith('+')) {
            className = 'diff-line-add';
        } else if (line.startsWith('-')) {
            className = 'diff-line-remove';
        } else if (line.startsWith('@@')) {
            className = 'diff-line-hunk';
        }
        return `<div class="${className}">${escapeHtml(line) || ' '}</div>`;
    }).join('');

    return `<div class="diff-content">${formattedLines}</div>`;
}

// Setup Synchronized Scrolling for Diff Panes
function setupDiffScrollSync() {
    const agentPane = document.getElementById('agent-diff-content');
    const goldPane = document.getElementById('gold-diff-content');

    if (!agentPane || !goldPane) return;

    let isSyncing = false;

    const syncScroll = (source, target) => {
        if (isSyncing) return;
        isSyncing = true;

        const sourceScrollPercentage = source.scrollTop / (source.scrollHeight - source.clientHeight);
        target.scrollTop = sourceScrollPercentage * (target.scrollHeight - target.clientHeight);

        setTimeout(() => { isSyncing = false; }, 10);
    };

    agentPane.addEventListener('scroll', () => syncScroll(agentPane, goldPane));
    goldPane.addEventListener('scroll', () => syncScroll(goldPane, agentPane));
}
