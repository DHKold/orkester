# New UI

## General design

The new UI is designed to be more intuitive and user-friendly, with a focus on simplicity and ease of use. The layout is clean and organized, with clear navigation and easy access to all features.

The main navigation menu is located on the left side of the screen.
- At the top of the menu is the application logo and title.
- Two importants sections are highlighted at the top of the menu: The authenticated user and the active Namespace.
- The authenticated user section displays the Logged User (with an avatar and name/id) and if the user is represeting a team, the team name/id is also displayed with its color. In the initial version, the logged user and team are mocked (hardcoded) for display purposes. In a future version, this will be replaced by a real authentication mechanism (e.g., JWT, OIDC proxy headers).
- The active Namespace section shows the currently selected Namespace, allowing users to easily switch between different Namespaces if needed. The active Namespace scopes all sections of the application, including both the Catalog and the Workspace, ensuring that users only see resources belonging to their selected Namespace.
- The menu has a vertical layout with icons and labels for each link. The links include Dashboard, Catalog, Workspace, Metrics, Settings, and Help.
- Each menu item is represented by an icon and a label, making it easy for users to quickly identify and navigate to the desired section of the application.
- The menu is designed to be responsive, adapting to different screen sizes and devices.
- When collapsed, the menu will only show icons, allowing for more screen space for the main content area. When expanded, both icons and labels are visible for easier navigation.
- When not collapsed, the menu can be resized by dragging the edge, allowing users to customize the width of the menu according to their preferences.

The main content area is located to the right of the navigation menu and is where users will interact with the various features and functionalities of the application.
- The content area is designed to be flexible and adaptable, allowing for different types of content to be displayed based on the user's actions and selections.
- The content area has a breadcrumb navigation at the top, providing users with a clear path of their current location within the application and allowing for easy navigation back to previous sections.

## Sections

### Dashboard

The Dashboard is the main landing page of the application, providing users with an overview of their activities and important information at a glance. It is designed to be visually appealing and informative, with a focus on key metrics and actionable insights. It uses cards and widgets to display information in a clear and organized manner, allowing users to quickly access the information they need and take action accordingly. The user can customize the dashboard by adding, removing, and rearranging the cards and widgets according to their preferences and needs. The widgets system is designed to use a grid system, so that multiple widgets can be displayed in a structured and organized way, allowing users to easily view and interact with the information presented on the dashboard. Widget layout preferences are persisted in the browser's local storage, ensuring that the dashboard configuration is retained across sessions without requiring a backend implementation. In the future, this can be synced with a `core/UserSettings` document in the Catalog when a backend settings model is available.

### Catalog

The Catalog is a section of the application that provides users with a comprehensive list of available documents. All documents follow the same base structure with:

- kind: the kind of the document, in the form of a string with the format `<group>/<kind>:<version>`. The group itself can use slashes for hierarchical organization (e.g., `catalog/Action` as a group). For example, `workaholic/Namespace:1.0`, `core/Namespace:1.2`, or `catalog/Action/Search:2.5` (where `catalog/Action` is the group, `Search` is the kind, and `2.5` is the version).
- name: the name of the document, which is a unique identifier within its kind. For example, `my-namespace` or `search-all`.
- version: the version of the document, which is a string that follows semantic versioning. For example, `1.0.0` or `2.5.1`.
- metadata: an object that contains additional information about the document, such as labels, annotations, and other relevant data. Known metadata fields (other can be added freely) include:
  - namespace: the namespace to which the document belongs, which is a string that identifies the logical grouping of documents. For example, `default` or `my-namespace`.
  - owner: the owner of the document, which is a string that identifies the user or team responsible for the document. For example, `user:john` or `team:frontend`.
  - description: a brief description of the document, which is a string that provides an overview of the document's purpose and functionality. For example, `This document defines the search action for all entities in the catalog.`.
  - tags: a list of strings that can be used to categorize and organize documents. For example, `env:production` or `team:frontend`.
- spec: an object that contains the specification of the document, which is defined by the kind of the document. The spec can have different fields and structures depending on the kind of the document. For example, a `catalog/Action` document might have a spec that includes fields such as `input`, `output`, and `handler`, while a `core/Namespace` document might have a spec that includes fields such as `description` and `owner`.
- status: an object that contains the status of the document, which is defined by the kind of the document. The status can have different fields and structures depending on the kind of the document. For example, a `catalog/Action` document might have a status that includes fields such as `lastExecuted` and `executionStatus`, while a `core/Namespace` document might have a status that includes fields such as `createdAt` and `updatedAt`.

The catalog provides users with a powerful search and filtering system, allowing them to quickly find the documents they need based on various criteria such as kind, name, version, metadata, and more. Users can also view detailed information about each document, including its specification and status, and can perform actions such as editing, deleting, or creating new documents directly from the catalog interface.

### Workspace

The Workspace is the section of the application where users can manage their Works, Tasks, Runnners, Crons and Runs. It's the main area built to handle the Workaholic plugin. (Reminder: the Workspace handles the execution while the catalog handles the storage of documents, including Works and Tasks).

The main pages of the Workspace are:
- Runners: Two-tabbed view listing WorkRunners and TaskRunners separately, each with filters and search capabilities.
  - WorkRunners: Each entry shows name, state, and current load (`active_work_runs` / `active_task_runs`). Click to view details. Global Actions: Create a new WorkRunner.
  - TaskRunners: Each entry shows name, kind (shell, container, kubernetes, http), and state. Click to view details. TaskRunners are read-only (managed by the system).
- WorkRuns: List of all the WorkRuns, with filters and search capabilities. Each WorkRun can be clicked to view its details. Global Actions: Start a new WorkRun (the underlying WorkRunRequest is created transparently by the UI).
- Crons: List of all the Crons, with filters and search capabilities. Each Cron can be clicked to view its details. Global Actions: Create a new Cron.
- WorkRun Details: Detailed view of a specific WorkRun, showing its specification, status, execution logs, and related information. Includes:
  - A DAG visualization of the workflow execution graph (step dependencies with live color-coded status indicators). A library such as Cytoscape.js or D3.js is used for this visualization.
  - The list of TaskRuns with full details, inputs/outputs, and inline logs (stdout and stderr displayed in separate tabs within a collapsible panel, fetched from the artifact URIs in `logsRef`).
- Cron Details: Detailed view of a specific Cron, showing its specification, status, and related information. Surfaces all available status fields: `last_scheduled_time`, `next_scheduled_time`, `last_run_status`, `consecutive_failures`, and `run_count`. Includes the list of WorkRuns triggered by the Cron.
- WorkRunner Details: Detailed view of a specific WorkRunner, showing its specification, status, and live load indicators (`active_work_runs` and `active_task_runs` displayed prominently with visual gauges or color indicators). Includes the list of WorkRuns executed by the WorkRunner.
- TaskRunner Details: Detailed view of a specific TaskRunner, showing its kind, state, state history, and metrics.

### Metrics

The Metrics section provides users with insights and analytics about everything that happens in the application. It's the interface to the Metrics Server, which collects and aggregates data from various sources within the application to provide users with valuable insights and analytics. The Metrics Server is designed to be flexible and extensible, allowing for the collection of a wide range of metrics and data points based on the needs of the application and its users.

### Settings

The Settings section allows users to configure various aspects of the application, including user preferences, account settings, and application configurations. It provides a centralized location for users to manage their settings and customize their experience within the application. The Settings section is designed to be intuitive and user-friendly, with clear navigation and organized categories for different types of settings. Users can easily access and modify their settings to tailor the application to their specific needs and preferences.

The various settings are stored as documents in the Catalog. For example, user preferences can be stored as `core/UserSettings` documents, while application configurations can be stored as `core/AppConfig` documents. Since the Catalog can store any document kind regardless of whether a formal backend schema exists for it, these kinds can be used as flexible, schemaless documents in the initial implementation. In a future iteration, formal schemas may be defined for automatic validation and richer UI rendering. Users can create, edit, and delete these settings documents directly from the Settings interface, providing a seamless experience for managing their preferences and configurations.

### Help

The Help section provides users with access to documentation, tutorials, FAQs, and support resources to assist them in using the application effectively. It serves as a comprehensive resource for users to find answers to their questions, learn how to use different features of the application, and get assistance when needed. The Help section is designed to be easily accessible and user-friendly, with clear navigation and organized content to help users quickly find the information they need. The Help section can include various types of content, such as written documentation, video tutorials, interactive guides, and links to support channels, providing users with multiple ways to access the information and assistance they need. Users can also search for specific topics or questions within the Help section, making it easier to find relevant information and resources.

All the content in the Help section can be stored as documents in the Catalog, such as `core/HelpTopic` documents, allowing for easy management and updating of help resources directly from the Catalog interface. Since the Catalog can store any document kind without requiring a formal backend schema, `core/HelpTopic` can be used as a flexible document kind in the initial implementation. This also allows for a consistent structure and format for all help-related content within the application.

## Design system

The design system for the new UI is based on a set of principles and guidelines that ensure consistency, usability, and accessibility across the entire application. The design system includes a comprehensive set of components, patterns, and styles that can be reused throughout the application to create a cohesive and unified user experience. The design system is built with a focus on modularity and flexibility, allowing for easy customization and adaptation to different use cases and user needs. The design system also incorporates best practices for accessibility, ensuring that the application is usable by a wide range of users, including those with disabilities. The design system is continuously evolving and improving based on user feedback and changing requirements, ensuring that it remains relevant and effective in meeting the needs of the application and its users.

No framework is used (Angular, React, Vue, etc.) to build the UI. The design system is implemented using standard web technologies such as HTML, CSS, and JavaScript, with a focus on creating lightweight and performant components that can be easily integrated into the application without relying on external dependencies. This approach allows for greater flexibility and control over the design and functionality of the UI, while also ensuring that it remains accessible and compatible across different browsers and devices.

External libraries are used only when necessary, and are carefully selected to ensure that they align with the design principles and goals of the application. For example, a library like D3.js might be used for complex data visualizations in the Metrics section, while a library like CodeMirror could be used for code editing features in the Catalog or Workspace sections. However, the core design and functionality of the UI is built using standard web technologies, ensuring that it remains lightweight, performant, and easy to maintain over time.

The design system includes a comprehensive set of components, such as buttons, forms, modals, cards, tables, and more, that can be easily reused and customized throughout the application. Each component is designed with accessibility in mind, following best practices for keyboard navigation, screen reader support, and color contrast. The design system also includes a set of patterns and guidelines for common UI elements and interactions, such as navigation menus, search bars, filters, and more, to ensure consistency and usability across the application. The design system is documented and maintained in a central repository, allowing for easy access and collaboration among the development team. The design system is continuously updated and improved based on user feedback and changing requirements, ensuring that it remains relevant and effective in meeting the needs of the application and its users.