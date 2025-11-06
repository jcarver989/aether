// State
let reportData = null;
let currentEval = null;
let expandedTraces = new Set();
let allExpanded = false;
let searchTerm = '';
let currentTab = 'assertions'; // 'assertions', 'traces', 'code_changes'

// Initialize
document.addEventListener('DOMContentLoaded', async () => {
    try {
        const response = await fetch('report-data.json');
        reportData = await response.json();
        renderSummary();
        renderEvalList();
        setupEventListeners();
    } catch (error) {
        console.error('Failed to load report data:', error);
        document.getElementById('empty-state').innerHTML =
            `<p style="color: var(--failure);">Failed to load report data: ${error.message}</p>`;
    }
});

// Event Listeners
function setupEventListeners() {
    const searchInput = document.getElementById('search-input');
    searchInput.addEventListener('input', (e) => {
        searchTerm = e.target.value.toLowerCase();
        renderTraces();
    });
}

// Render Summary Stats
function renderSummary() {
    const { summary } = reportData;
    const statsHtml = `
        <div class="stat">
            <div class="stat-label">Evals</div>
            <div class="stat-value">
                <span class="success">${summary.passed_evals}</span> /
                <span class="failure">${summary.failed_evals}</span> /
                ${summary.total_evals}
            </div>
        </div>
        <div class="stat">
            <div class="stat-label">Assertions</div>
            <div class="stat-value">
                <span class="success">${summary.passed_assertions}</span> /
                <span class="failure">${summary.failed_assertions}</span> /
                ${summary.total_assertions}
            </div>
        </div>
        <div class="stat">
            <div class="stat-label">Success Rate</div>
            <div class="stat-value ${summary.passed_evals === summary.total_evals ? 'success' : 'failure'}">
                ${summary.total_evals > 0 ? Math.round((summary.passed_evals / summary.total_evals) * 100) : 0}%
            </div>
        </div>
    `;
    document.getElementById('summary-stats').innerHTML = statsHtml;
}

// Render Eval List
function renderEvalList() {
    const { summary } = reportData;
    const evalListHtml = summary.evals.map(eval => {
        const status = eval.passed ? '✓' : '✗';
        const statusColor = eval.passed ? 'success' : 'failure';
        const assertionsPassed = eval.assertions.filter(a => a.passed).length;
        const totalAssertions = eval.assertions.length;

        return `
            <div class="eval-item" data-eval="${escapeHtml(eval.eval_name)}" onclick="selectEval('${escapeHtml(eval.eval_name)}')">
                <div class="eval-status" style="color: var(--${statusColor})">${status}</div>
                <div style="flex: 1; min-width: 0;">
                    <div class="eval-name">${escapeHtml(eval.eval_name)}</div>
                    <div class="eval-meta">${assertionsPassed}/${totalAssertions} assertions passed</div>
                </div>
            </div>
        `;
    }).join('');

    document.getElementById('eval-list').innerHTML = evalListHtml;
}

// Select Eval
function selectEval(evalName) {
    currentEval = evalName;
    expandedTraces.clear();
    allExpanded = false;
    currentTab = 'assertions'; // Reset to default tab

    // Update active state in sidebar
    document.querySelectorAll('.eval-item').forEach(item => {
        item.classList.toggle('active', item.dataset.eval === evalName);
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
    const eval = reportData.summary.evals.find(e => e.eval_name === currentEval);
    if (!eval) return;

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
            <h2 class="eval-title">${escapeHtml(currentEval)}</h2>
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

// Render Traces
function renderTraces() {
    const traces = reportData.eval_traces[currentEval] || [];

    // Filter traces by search term
    const filteredTraces = traces.filter(trace => {
        if (!searchTerm) return true;
        const searchableText = `${trace.message} ${trace.level} ${trace.target}`.toLowerCase();
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

        // Check if there are extra details to show
        const hasDetails = Object.keys(trace.extra || {}).length > 0 || trace.target;

        return `
            <div class="trace-event">
                <div class="trace-marker ${level}"></div>
                <div class="trace-content">
                    <div class="trace-header" onclick="${hasDetails ? `toggleTrace('${traceId}')` : ''}">
                        <span class="trace-level ${level}">${escapeHtml(trace.level)}</span>
                        <span class="trace-timestamp">${relativeTime}</span>
                        <span class="trace-message">${escapeHtml(trace.message)}</span>
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
        ...trace.extra
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
    renderTraces();
}

// Toggle Expand All
function toggleExpandAll() {
    allExpanded = !allExpanded;
    expandedTraces.clear();

    if (allExpanded) {
        const traces = reportData.eval_traces[currentEval] || [];
        traces.forEach((_, index) => {
            expandedTraces.add(`trace-${index}`);
        });
    }

    renderEvalDetails();
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
