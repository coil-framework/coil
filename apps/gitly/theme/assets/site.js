const GITLY = (() => {
  const translations = {
    "en-GB": {
      brand: "Gitly",
      searchPlaceholder: "Search repositories, users, docs, and workflows",
      nav: { home: "Home", explore: "Explore", repo: "Repository", pulls: "Pull requests", actions: "Actions", org: "Organization", profile: "Profile" },
      controls: {
        language: "Language",
        theme: "Theme",
        light: "Light",
        dark: "Dark",
        system: "System",
        search: "Search",
        searchSubmit: "Search",
        skip: "Skip to content",
        primaryNavigation: "Primary navigation",
        repositoryNavigation: "Repository navigation",
        repositorySummary: "Repository summary"
      },
      footer: "Gitly is a customer-root Davenda demo showing CMS, custom API surfaces, linked Rust hooks, and runtime-installed WASM.",
      pageTitles: { home: "Gitly · Home", explore: "Gitly · Explore", repo: "forgeflow/platform-ui", issues: "Issues · forgeflow/platform-ui", pulls: "Pull requests · forgeflow/platform-ui", actions: "Actions · forgeflow/platform-ui", org: "Forgeflow", profile: "Alex Mariner", search: "Search · Gitly" },
      copy: {
        "home.eyebrow": "Customer-root GitHub-style demo",
        "home.title": "One Davenda app can look like a forge, not just a storefront.",
        "home.summary": "Gitly is a multilingual, theme-switchable, accessible demo with linked Rust business rules, custom API endpoints, and runtime-installed WASM.",
        "home.primary": "Open repository",
        "home.secondary": "View Actions",
        "home.feed": "Recent activity",
        "home.activity": "Contribution graph",
        "home.pulse": "Community pulse",
        "home.pulseBody": "This card is hydrated from a custom API surface. One endpoint is extended through the bounded WASM path.",
        "home.extensionLabel": "Extension",
        "home.statusLabel": "Status",
        "home.runtimeLabel": "Runtime",
        "home.runtimeValue": "WASM API surface",
        "home.item1": "alexmariner merged ARIA navigation updates into main",
        "home.item2": "Localization smoke refreshed French and German strings",
        "home.item3": "Actions scheduler queued the next UI regression pass",
        "home.graphBody": "Static repositories, users, and workflow runs create the GitHub feel without pretending there is a real git engine underneath.",
        "home.graphSummary": "Contribution activity is shown as a decorative weekly sparkline. This demo does not expose detailed contribution counts.",
        "home.weekdays": "Mon Tue Wed Thu Fri Sat Sun",
        "home.stars": "Stars",
        "home.forks": "Forks",
        "home.openIssues": "Open issues",
        "home.workflowRuns": "Workflow runs",
        "home.apiFallback": "API hydration failed. The static fallback content is still available.",
        "explore.title": "Explore repositories and teams",
        "explore.summary": "Trending demo surfaces show how Davenda can present developer products, docs, and automation under one customer app.",
        "explore.trending": "Trending this week",
        "explore.events": "Modular highlights",
        "explore.item1": "Accessible repository shell and multilingual frontend demo.",
        "explore.item2": "Markdown import, CMS publishing, and policy hooks in one workspace.",
        "explore.item3": "Operational dashboards layered over the same customer-root runtime.",
        "explore.highlight1": "Customer-owned Rust backend policies",
        "explore.highlight2": "Custom GitHub-style API endpoints",
        "explore.highlight3": "Runtime-installed WASM API and scheduler demo",
        "explore.highlight4": "Accessible theme and language switching",
        "repo.summary": "Accessible multilingual UI primitives and customer-app examples for Davenda.",
        "repo.readmeTitle": "README",
        "repo.readmeBody": "Gitly demonstrates a customer-owned workspace using Davenda as an upstream dependency. Linked Rust hooks govern editorial policy, custom routes expose GitHub-like APIs, and bounded WASM packages extend selected runtime surfaces.",
        "repo.about": "About this repository",
        "repo.reviewPulls": "Review open pull requests",
        "repo.inspectActions": "Inspect workflow runs",
        "repo.meta.stars": "Stars",
        "repo.meta.forks": "Forks",
        "repo.meta.watchers": "Watchers",
        "repo.meta.issues": "Issues",
        "repo.meta.language": "Language",
        "repo.meta.license": "License",
        "repo.code": "Code", "repo.issues": "Issues", "repo.pulls": "Pull requests", "repo.actions": "Actions",
        "issues.title": "Open issues",
        "issues.summary": "Static issue cards keep the GitHub-shaped navigation loop intact while staying honest that this demo does not ship a full issue tracker.",
        "issues.caption": "Open issues for forgeflow/platform-ui",
        "issues.head.issue": "Issue",
        "issues.head.owner": "Owner",
        "issues.head.labels": "Labels",
        "issues.head.status": "Status",
        "issues.label.a11y": "accessibility",
        "issues.label.i18n": "localization",
        "issues.label.design": "design-system",
        "issues.status.triage": "Needs triage",
        "issues.status.inProgress": "In progress",
        "issues.status.ready": "Ready for review",
        "pulls.title": "Open pull requests",
        "pulls.summary": "Static pull requests mimic GitHub review flow while keeping the demo honest about what is and is not implemented.",
        "pulls.caption": "Open pull requests for forgeflow/platform-ui",
        "pulls.head.pr": "Pull request", "pulls.head.author": "Author", "pulls.head.checks": "Checks", "pulls.head.status": "Status",
        "pulls.status.review": "Review required",
        "pulls.status.passed": "Checks passed",
        "pulls.status.draft": "Draft",
        "actions.title": "Workflow runs",
        "actions.summary": "Workflow rows remain fixture data, but the scheduler panel below is derived from the built runtime plan and the installed scheduled-job extension.",
        "actions.schedule": "Scheduled automation",
        "actions.scheduleBody": "The `github.actions.refresh` surface is declared by the Gitly customer module and can be fulfilled by a runtime-installed scheduled-job extension.",
        "actions.extension": "WASM runtime story",
        "actions.extensionBody": "Gitly ships both a custom API extension and a scheduled-job extension to show how third-party behavior stays bounded at runtime.",
        "actions.runtimeTitle": "Runtime scheduler state",
        "actions.runtimeBody": "These values come from the Gitly runtime plan and registered scheduled-job handler, not browser local storage.",
        "actions.runtimeContract": "Contract",
        "actions.runtimeModule": "Module",
        "actions.runtimeExtension": "Extension",
        "actions.runtimeHandler": "Handler",
        "actions.runtimeTrigger": "Trigger",
        "actions.runtimeSchedule": "Schedule",
        "actions.runtimeQueue": "Queue",
        "actions.runtimeBackend": "Backend",
        "actions.runtimeRetryLimit": "Retry limit",
        "actions.runtimeDeadLetter": "Dead-letter queue",
        "actions.runtimeScheduledJobs": "Registered scheduled jobs",
        "actions.runtimeHandlerCount": "Installed handlers",
        "actions.job2": "Localization smoke",
        "actions.job2Cadence": "Every hour",
        "actions.job3": "WASM extension contract",
        "actions.job3Cadence": "On push and PR",
        "actions.meta.running": "Running",
        "actions.meta.queued": "Queued",
        "actions.meta.total": "Total workflows",
        "org.title": "Forgeflow organization",
        "org.summary": "A fictional product engineering group showing reusable branding, developer workflows, and docs operations on Davenda.",
        "org.repositories": "Highlighted repositories",
        "org.people": "Core maintainers",
        "org.repo1": "Design system and customer-app examples",
        "org.repo2": "Versioned docs and editorial governance",
        "org.repo3": "Internal operations and release visibility",
        "profile.title": "Alex Mariner",
        "profile.summary": "Staff Engineer focused on accessibility, multilingual delivery, and workflow tooling.",
        "profile.activity": "Recent contributions",
        "profile.pinned": "Pinned repositories",
        "profile.activity1": "Published navigation accessibility guidance for repository shells",
        "profile.activity2": "Reviewed the French and German language switcher rollout",
        "profile.activity3": "Validated the bounded WASM API extension contract",
        "search.title": "Search results",
        "search.summary": "The search surface is static-but-usable: it filters the checked-in demo repositories, docs, users, and Actions pages by the current query.",
        "search.queryLabel": "Query",
        "search.empty": "No demo results matched this query.",
        "search.type.repo": "Repository",
        "search.type.docs": "Documentation",
        "search.type.person": "Person",
        "search.type.actions": "Actions",
        "search.result.repo": "Accessible multilingual UI primitives and customer-app examples for Davenda.",
        "search.result.docs": "Versioned docs and editorial policy examples running in the same customer-root workspace.",
        "search.result.profile": "Staff Engineer focused on accessibility, multilingual delivery, and workflow tooling.",
        "search.result.actions": "Workflow rows stay fixture-backed, while scheduler contract and handler details come from the runtime job plan.",
        "api.visibility.public": "Public",
        "api.status.active": "active",
        "api.workflow.primary": "UI regression",
        "api.workflow.primaryCadence": "Every 30 minutes"
      }
    },
    "fr-FR": {
      brand: "Gitly",
      searchPlaceholder: "Rechercher des dépôts, utilisateurs, docs et workflows",
      nav: { home: "Accueil", explore: "Explorer", repo: "Dépôt", pulls: "Pull requests", actions: "Actions", org: "Organisation", profile: "Profil" },
      controls: {
        language: "Langue",
        theme: "Thème",
        light: "Clair",
        dark: "Sombre",
        system: "Système",
        search: "Rechercher",
        searchSubmit: "Rechercher",
        skip: "Aller au contenu",
        primaryNavigation: "Navigation principale",
        repositoryNavigation: "Navigation du dépôt",
        repositorySummary: "Résumé du dépôt"
      },
      footer: "Gitly est une démo Davenda en espace client montrant le CMS, des API sur mesure, des hooks Rust liés et du WASM installé à l'exécution.",
      pageTitles: { home: "Gitly · Accueil", explore: "Gitly · Explorer", repo: "forgeflow/platform-ui", issues: "Issues · forgeflow/platform-ui", pulls: "Pull requests · forgeflow/platform-ui", actions: "Actions · forgeflow/platform-ui", org: "Forgeflow", profile: "Alex Mariner", search: "Recherche · Gitly" },
      copy: {
        "home.eyebrow": "Démo GitHub en espace client",
        "home.title": "Une application Davenda peut ressembler à une forge, pas seulement à une boutique.",
        "home.summary": "Gitly est une démo multilingue, accessible et à thèmes commutables avec logique Rust liée, API personnalisées et WASM borné.",
        "home.primary": "Ouvrir le dépôt",
        "home.secondary": "Voir les Actions",
        "home.feed": "Activité récente",
        "home.activity": "Graphe de contribution",
        "home.pulse": "Pouls de la communauté",
        "home.pulseBody": "Cette carte est alimentée par une API personnalisée. Un point de terminaison est étendu via le chemin WASM borné.",
        "home.extensionLabel": "Extension",
        "home.statusLabel": "Statut",
        "home.runtimeLabel": "Exécution",
        "home.runtimeValue": "Surface API WASM",
        "home.item1": "alexmariner a fusionné les améliorations ARIA de navigation vers main",
        "home.item2": "Le test de localisation a actualisé les chaînes françaises et allemandes",
        "home.item3": "Le planificateur Actions a mis en file la prochaine passe de régression UI",
        "home.graphBody": "Des dépôts, utilisateurs et exécutions de workflow statiques recréent l’ambiance GitHub sans prétendre qu’un vrai moteur git existe.",
        "home.graphSummary": "L’activité des contributions est présentée comme une courbe décorative sur la semaine. Cette démo n’expose pas de comptage détaillé des contributions.",
        "home.weekdays": "Lun Mar Mer Jeu Ven Sam Dim",
        "home.stars": "Étoiles",
        "home.forks": "Forks",
        "home.openIssues": "Issues ouvertes",
        "home.workflowRuns": "Exécutions de workflow",
        "home.apiFallback": "Le chargement API a échoué. Le contenu statique de secours reste disponible.",
        "explore.title": "Explorer les dépôts et équipes",
        "explore.summary": "Les surfaces tendance montrent comment Davenda peut présenter produits développeur, documentation et automatisation dans une seule application client.",
        "explore.trending": "Tendance cette semaine",
        "explore.events": "Points forts modulaires",
        "explore.item1": "Démo de dépôt accessible avec frontend multilingue.",
        "explore.item2": "Import Markdown, publication CMS et hooks de politique dans un seul espace de travail.",
        "explore.item3": "Tableaux de bord opérationnels superposés au même runtime client.",
        "explore.highlight1": "Politiques backend Rust détenues par le client",
        "explore.highlight2": "API personnalisées de style GitHub",
        "explore.highlight3": "Démo d’API WASM et de planification installées à l’exécution",
        "explore.highlight4": "Changement accessible du thème et de la langue",
        "repo.summary": "Primitives UI accessibles et multilingues ainsi qu’exemples d’applications clientes pour Davenda.",
        "repo.readmeTitle": "README",
        "repo.readmeBody": "Gitly montre un espace de travail client qui consomme Davenda comme dépendance amont. Les hooks Rust liés gouvernent la politique éditoriale, des routes personnalisées exposent des API de style GitHub, et des paquets WASM bornés étendent certaines surfaces d’exécution.",
        "repo.about": "À propos de ce dépôt",
        "repo.reviewPulls": "Examiner les pull requests ouvertes",
        "repo.inspectActions": "Inspecter les workflows",
        "repo.meta.stars": "Étoiles",
        "repo.meta.forks": "Forks",
        "repo.meta.watchers": "Observateurs",
        "repo.meta.issues": "Issues",
        "repo.meta.language": "Langage",
        "repo.meta.license": "Licence",
        "repo.code": "Code", "repo.issues": "Issues", "repo.pulls": "Pull requests", "repo.actions": "Actions",
        "issues.title": "Issues ouvertes",
        "issues.summary": "Des cartes d’issues statiques gardent la navigation de style GitHub complète tout en restant honnêtes: cette démo ne fournit pas un véritable gestionnaire d’issues.",
        "issues.caption": "Issues ouvertes pour forgeflow/platform-ui",
        "issues.head.issue": "Issue",
        "issues.head.owner": "Responsable",
        "issues.head.labels": "Labels",
        "issues.head.status": "Statut",
        "issues.label.a11y": "accessibilité",
        "issues.label.i18n": "localisation",
        "issues.label.design": "design-system",
        "issues.status.triage": "À trier",
        "issues.status.inProgress": "En cours",
        "issues.status.ready": "Prêt pour revue",
        "pulls.title": "Pull requests ouvertes",
        "pulls.summary": "Des pull requests statiques reproduisent le flux de revue GitHub tout en restant honnêtes sur ce qui est réellement implémenté.",
        "pulls.caption": "Pull requests ouvertes pour forgeflow/platform-ui",
        "pulls.head.pr": "Pull request", "pulls.head.author": "Auteur", "pulls.head.checks": "Checks", "pulls.head.status": "Statut",
        "pulls.status.review": "Revue requise",
        "pulls.status.passed": "Checks validés",
        "pulls.status.draft": "Brouillon",
        "actions.title": "Exécutions de workflow",
        "actions.summary": "Les lignes de workflow restent des fixtures, mais le panneau du planificateur ci-dessous est dérivé du plan d’exécution construit et de l’extension de tâche planifiée installée.",
        "actions.schedule": "Automatisation planifiée",
        "actions.scheduleBody": "La surface `github.actions.refresh` est déclarée par le module client Gitly et peut être remplie par une extension WASM planifiée.",
        "actions.extension": "Chemin WASM à l’exécution",
        "actions.extensionBody": "Gitly fournit une extension API personnalisée et une extension de tâche planifiée pour montrer comment le comportement tiers reste borné à l’exécution.",
        "actions.runtimeTitle": "État du planificateur à l’exécution",
        "actions.runtimeBody": "Ces valeurs proviennent du plan d’exécution Gitly et du handler de tâche planifiée enregistré, pas du stockage local du navigateur.",
        "actions.runtimeContract": "Contrat",
        "actions.runtimeModule": "Module",
        "actions.runtimeExtension": "Extension",
        "actions.runtimeHandler": "Handler",
        "actions.runtimeTrigger": "Déclencheur",
        "actions.runtimeSchedule": "Planification",
        "actions.runtimeQueue": "File",
        "actions.runtimeBackend": "Backend",
        "actions.runtimeRetryLimit": "Limite de reprise",
        "actions.runtimeDeadLetter": "File de lettres mortes",
        "actions.runtimeScheduledJobs": "Tâches planifiées enregistrées",
        "actions.runtimeHandlerCount": "Handlers installés",
        "actions.job2": "Vérification de localisation",
        "actions.job2Cadence": "Chaque heure",
        "actions.job3": "Contrat d’extension WASM",
        "actions.job3Cadence": "À chaque push et pull request",
        "actions.meta.running": "En cours",
        "actions.meta.queued": "En file",
        "actions.meta.total": "Workflows au total",
        "org.title": "Organisation Forgeflow",
        "org.summary": "Un groupe fictif d’ingénierie produit montrant image de marque réutilisable, flux développeur et opérations de documentation sur Davenda.",
        "org.repositories": "Dépôts mis en avant",
        "org.people": "Mainteneurs principaux",
        "org.repo1": "Design system et exemples d’applications clientes",
        "org.repo2": "Documentation versionnée et gouvernance éditoriale",
        "org.repo3": "Opérations internes et visibilité des releases",
        "profile.title": "Alex Mariner",
        "profile.summary": "Staff Engineer axé sur l’accessibilité, la livraison multilingue et l’outillage de workflows.",
        "profile.activity": "Contributions récentes",
        "profile.pinned": "Dépôts épinglés",
        "profile.activity1": "Publication des directives d’accessibilité de navigation pour les interfaces de dépôt",
        "profile.activity2": "Revue du déploiement du sélecteur de langue français et allemand",
        "profile.activity3": "Validation du contrat d’extension API WASM bornée",
        "search.title": "Résultats de recherche",
        "search.summary": "La recherche reste statique mais utilisable: elle filtre les dépôts, la documentation, les profils et les surfaces Actions inclus dans la démo.",
        "search.queryLabel": "Requête",
        "search.empty": "Aucun résultat de démonstration ne correspond à cette requête.",
        "search.type.repo": "Dépôt",
        "search.type.docs": "Documentation",
        "search.type.person": "Profil",
        "search.type.actions": "Actions",
        "search.result.repo": "Primitives UI accessibles et multilingues ainsi qu’exemples d’applications clientes pour Davenda.",
        "search.result.docs": "Documentation versionnée et exemples de politique éditoriale dans le même espace client.",
        "search.result.profile": "Staff Engineer axé sur l’accessibilité, la livraison multilingue et l’outillage de workflows.",
        "search.result.actions": "Les lignes de workflow restent des fixtures, tandis que le contrat et les handlers du planificateur viennent du plan de jobs d’exécution.",
        "api.visibility.public": "Public",
        "api.status.active": "actif",
        "api.workflow.primary": "Régression UI",
        "api.workflow.primaryCadence": "Toutes les 30 minutes"
      }
    },
    "de-DE": {
      brand: "Gitly",
      searchPlaceholder: "Repositorys, Nutzer, Doku und Workflows durchsuchen",
      nav: { home: "Start", explore: "Entdecken", repo: "Repository", pulls: "Pull Requests", actions: "Actions", org: "Organisation", profile: "Profil" },
      controls: {
        language: "Sprache",
        theme: "Design",
        light: "Hell",
        dark: "Dunkel",
        system: "System",
        search: "Suchen",
        searchSubmit: "Suchen",
        skip: "Zum Inhalt springen",
        primaryNavigation: "Hauptnavigation",
        repositoryNavigation: "Repository-Navigation",
        repositorySummary: "Repository-Zusammenfassung"
      },
      footer: "Gitly ist eine Davenda-Demo im Customer-Root-Workspace mit CMS, eigenen APIs, eingebundenen Rust-Hooks und zur Laufzeit installiertem WASM.",
      pageTitles: { home: "Gitly · Start", explore: "Gitly · Entdecken", repo: "forgeflow/platform-ui", issues: "Issues · forgeflow/platform-ui", pulls: "Pull Requests · forgeflow/platform-ui", actions: "Actions · forgeflow/platform-ui", org: "Forgeflow", profile: "Alex Mariner", search: "Suche · Gitly" },
      copy: {
        "home.eyebrow": "GitHub-ähnliche Customer-Root-Demo",
        "home.title": "Eine Davenda-Anwendung kann wie eine Forge aussehen, nicht nur wie ein Store.",
        "home.summary": "Gitly ist eine mehrsprachige, zugängliche und themenumschaltbare Demo mit eingebundener Rust-Logik, eigenen APIs und begrenztem WASM.",
        "home.primary": "Repository öffnen",
        "home.secondary": "Actions ansehen",
        "home.feed": "Letzte Aktivität",
        "home.activity": "Beitragsgraph",
        "home.pulse": "Community-Pulse",
        "home.pulseBody": "Diese Karte wird aus einer eigenen API geladen. Ein Endpunkt wird über den begrenzten WASM-Pfad erweitert.",
        "home.extensionLabel": "Erweiterung",
        "home.statusLabel": "Status",
        "home.runtimeLabel": "Laufzeit",
        "home.runtimeValue": "WASM-API-Fläche",
        "home.item1": "alexmariner hat ARIA-Navigationsupdates nach main gemergt",
        "home.item2": "Der Lokalisierungs-Smoketest hat französische und deutsche Texte aktualisiert",
        "home.item3": "Der Actions-Planer hat den nächsten UI-Regressionlauf eingeplant",
        "home.graphBody": "Statische Repositorys, Nutzer und Workflow-Läufe erzeugen das GitHub-Gefühl, ohne einen echten Git-Backend vorzutäuschen.",
        "home.graphSummary": "Die Beitragsaktivität wird als dekorative Wochenübersicht dargestellt. Diese Demo zeigt keine detaillierten Beitragszahlen an.",
        "home.weekdays": "Mo Di Mi Do Fr Sa So",
        "home.stars": "Sterne",
        "home.forks": "Forks",
        "home.openIssues": "Offene Issues",
        "home.workflowRuns": "Workflow-Läufe",
        "home.apiFallback": "Die API-Hydrierung ist fehlgeschlagen. Die statischen Ersatzinhalte bleiben verfügbar.",
        "explore.title": "Repositorys und Teams entdecken",
        "explore.summary": "Trendflächen zeigen, wie Davenda Entwicklerprodukte, Dokumentation und Automatisierung in einer Kundenanwendung bündeln kann.",
        "explore.trending": "Diese Woche im Trend",
        "explore.events": "Modulare Highlights",
        "explore.item1": "Zugängliche Repository-Hülle mit mehrsprachigem Frontend.",
        "explore.item2": "Markdown-Import, CMS-Publishing und Richtlinien-Hooks in einem Workspace.",
        "explore.item3": "Betriebs-Dashboards auf derselben Customer-Root-Laufzeit.",
        "explore.highlight1": "Kundeneigene Rust-Backend-Richtlinien",
        "explore.highlight2": "Eigene GitHub-artige API-Endpunkte",
        "explore.highlight3": "Zur Laufzeit installierte WASM-API- und Scheduler-Demo",
        "explore.highlight4": "Barrierefreier Theme- und Sprachwechsel",
        "repo.summary": "Zugängliche mehrsprachige UI-Bausteine und Customer-App-Beispiele für Davenda.",
        "repo.readmeTitle": "README",
        "repo.readmeBody": "Gitly zeigt einen kunden-eigenen Workspace, der Davenda als Upstream-Abhängigkeit nutzt. Eingebundene Rust-Hooks steuern redaktionelle Richtlinien, eigene Routen liefern GitHub-artige APIs und begrenzte WASM-Pakete erweitern ausgewählte Laufzeitflächen.",
        "repo.about": "Über dieses Repository",
        "repo.reviewPulls": "Offene Pull Requests prüfen",
        "repo.inspectActions": "Workflow-Läufe ansehen",
        "repo.meta.stars": "Sterne",
        "repo.meta.forks": "Forks",
        "repo.meta.watchers": "Beobachter",
        "repo.meta.issues": "Issues",
        "repo.meta.language": "Sprache",
        "repo.meta.license": "Lizenz",
        "repo.code": "Code", "repo.issues": "Issues", "repo.pulls": "Pull Requests", "repo.actions": "Actions",
        "issues.title": "Offene Issues",
        "issues.summary": "Statische Issue-Karten halten die GitHub-ähnliche Navigation vollständig, ohne einen echten Issue-Tracker vorzutäuschen.",
        "issues.caption": "Offene Issues für forgeflow/platform-ui",
        "issues.head.issue": "Issue",
        "issues.head.owner": "Verantwortlich",
        "issues.head.labels": "Labels",
        "issues.head.status": "Status",
        "issues.label.a11y": "Barrierefreiheit",
        "issues.label.i18n": "Lokalisierung",
        "issues.label.design": "Design-System",
        "issues.status.triage": "Braucht Triage",
        "issues.status.inProgress": "In Arbeit",
        "issues.status.ready": "Bereit für Review",
        "pulls.title": "Offene Pull Requests",
        "pulls.summary": "Statische Pull Requests bilden den GitHub-Review-Fluss nach und bleiben dabei ehrlich über den tatsächlichen Implementierungsumfang.",
        "pulls.caption": "Offene Pull Requests für forgeflow/platform-ui",
        "pulls.head.pr": "Pull Request", "pulls.head.author": "Autor", "pulls.head.checks": "Checks", "pulls.head.status": "Status",
        "pulls.status.review": "Review erforderlich",
        "pulls.status.passed": "Checks bestanden",
        "pulls.status.draft": "Entwurf",
        "actions.title": "Workflow-Läufe",
        "actions.summary": "Die Workflow-Zeilen bleiben Fixture-Daten, aber das Scheduler-Panel unten wird aus dem gebauten Runtime-Plan und der installierten Scheduled-Job-Erweiterung abgeleitet.",
        "actions.schedule": "Geplante Automatisierung",
        "actions.scheduleBody": "Die Fläche `github.actions.refresh` wird vom Gitly-Kundenmodul deklariert und kann von einer installierten Scheduled-Job-Erweiterung bedient werden.",
        "actions.extension": "WASM zur Laufzeit",
        "actions.extensionBody": "Gitly liefert sowohl eine eigene API-Erweiterung als auch eine geplante Job-Erweiterung, um begrenztes Drittanbieter-Verhalten zur Laufzeit zu zeigen.",
        "actions.runtimeTitle": "Runtime-Scheduler-Status",
        "actions.runtimeBody": "Diese Werte stammen aus dem Gitly-Runtime-Plan und dem registrierten Scheduled-Job-Handler, nicht aus dem lokalen Browser-Speicher.",
        "actions.runtimeContract": "Vertrag",
        "actions.runtimeModule": "Modul",
        "actions.runtimeExtension": "Erweiterung",
        "actions.runtimeHandler": "Handler",
        "actions.runtimeTrigger": "Auslöser",
        "actions.runtimeSchedule": "Zeitplan",
        "actions.runtimeQueue": "Queue",
        "actions.runtimeBackend": "Backend",
        "actions.runtimeRetryLimit": "Retry-Limit",
        "actions.runtimeDeadLetter": "Dead-Letter-Queue",
        "actions.runtimeScheduledJobs": "Registrierte Scheduled Jobs",
        "actions.runtimeHandlerCount": "Installierte Handler",
        "actions.job2": "Lokalisierungs-Smoketest",
        "actions.job2Cadence": "Jede Stunde",
        "actions.job3": "WASM-Erweiterungsvertrag",
        "actions.job3Cadence": "Bei Push und Pull Request",
        "actions.meta.running": "Läuft",
        "actions.meta.queued": "In Warteschlange",
        "actions.meta.total": "Workflows gesamt",
        "org.title": "Forgeflow-Organisation",
        "org.summary": "Ein fiktives Produktteam, das wiederverwendbares Branding, Entwicklerabläufe und Dokumentationsbetrieb auf Davenda zeigt.",
        "org.repositories": "Hervorgehobene Repositorys",
        "org.people": "Kernmaintainer",
        "org.repo1": "Designsystem und Customer-App-Beispiele",
        "org.repo2": "Versionierte Dokumentation und redaktionelle Governance",
        "org.repo3": "Interner Betrieb und Release-Sichtbarkeit",
        "profile.title": "Alex Mariner",
        "profile.summary": "Staff Engineer mit Fokus auf Barrierefreiheit, Mehrsprachigkeit und Workflow-Werkzeuge.",
        "profile.activity": "Letzte Beiträge",
        "profile.pinned": "Angeheftete Repositorys",
        "profile.activity1": "Navigationsrichtlinien für barrierefreie Repository-Oberflächen veröffentlicht",
        "profile.activity2": "Einführung des französischen und deutschen Sprachumschalters überprüft",
        "profile.activity3": "Vertrag der begrenzten WASM-API-Erweiterung validiert",
        "search.title": "Suchergebnisse",
        "search.summary": "Die Suche bleibt statisch, ist aber benutzbar: Sie filtert die eingecheckten Repository-, Doku-, Profil- und Actions-Demooberflächen nach der aktuellen Anfrage.",
        "search.queryLabel": "Suchbegriff",
        "search.empty": "Keine Demo-Ergebnisse passen zu dieser Anfrage.",
        "search.type.repo": "Repository",
        "search.type.docs": "Dokumentation",
        "search.type.person": "Profil",
        "search.type.actions": "Actions",
        "search.result.repo": "Zugängliche mehrsprachige UI-Bausteine und Customer-App-Beispiele für Davenda.",
        "search.result.docs": "Versionierte Dokumentation und redaktionelle Richtlinien im selben Customer-Root-Workspace.",
        "search.result.profile": "Staff Engineer mit Fokus auf Barrierefreiheit, Mehrsprachigkeit und Workflow-Werkzeuge.",
        "search.result.actions": "Workflow-Zeilen bleiben Fixture-Daten, während Scheduler-Vertrag und Handler-Details aus dem Runtime-Job-Plan stammen.",
        "api.visibility.public": "Öffentlich",
        "api.status.active": "aktiv",
        "api.workflow.primary": "UI-Regression",
        "api.workflow.primaryCadence": "Alle 30 Minuten"
      }
    }
  };

  const routes = {
    home: "",
    explore: "/explore",
    repo: "/forgeflow/platform-ui",
    issues: "/forgeflow/platform-ui/issues",
    pulls: "/forgeflow/platform-ui/pulls",
    actions: "/forgeflow/platform-ui/actions",
    org: "/orgs/forgeflow",
    profile: "/alexmariner",
    search: "/search"
  };

  const searchIndex = [
    { type: "repo", route: "repo", title: "forgeflow/platform-ui", summaryKey: "search.result.repo", terms: ["platform", "repo", "ui", "rust", "davenda", "accessibility", "multilingual"] },
    { type: "docs", route: "explore", title: "forgeflow/docs-portal", summaryKey: "search.result.docs", terms: ["docs", "documentation", "cms", "markdown", "policy", "editorial"] },
    { type: "person", route: "profile", title: "alexmariner", summaryKey: "search.result.profile", terms: ["alex", "profile", "accessibility", "language", "workflow"] },
    { type: "actions", route: "actions", title: "github.actions.refresh", summaryKey: "search.result.actions", terms: ["actions", "scheduler", "wasm", "workflow", "automation"] }
  ];

  const localePrefixes = { "en-GB": "", "fr-FR": "/fr", "de-DE": "/de" };

  function currentLocale() {
    const path = window.location.pathname;
    if (path === "/fr" || path.startsWith("/fr/")) return "fr-FR";
    if (path === "/de" || path.startsWith("/de/")) return "de-DE";
    return "en-GB";
  }

  function currentRoute() {
    return document.body.dataset.route || "home";
  }

  function localizePath(routeKey, locale) {
    return `${localePrefixes[locale]}${routes[routeKey]}` || "/";
  }

  function applyLinks(locale) {
    const currentQuery = currentRoute() === "search" ? window.location.search : "";
    document.querySelectorAll("[data-route-link]").forEach((link) => {
      const routeKey = link.getAttribute("data-route-link");
      const href = localizePath(routeKey, locale);
      link.setAttribute("href", href || "/");
      if (routeKey === currentRoute()) {
        link.setAttribute("aria-current", "page");
      } else {
        link.removeAttribute("aria-current");
      }
    });

    document.querySelectorAll("[data-language-link]").forEach((link) => {
      const target = link.getAttribute("data-language-link");
      link.setAttribute("href", `${localizePath(currentRoute(), target)}${currentQuery}`);
      link.setAttribute("lang", target);
      link.setAttribute("hreflang", target);
      if (target === locale) {
        link.setAttribute("aria-current", "page");
      } else {
        link.removeAttribute("aria-current");
      }
    });
  }

  function applySearchForms(locale) {
    const query = new URLSearchParams(window.location.search).get("q") || "";
    document.querySelectorAll("[data-search-form]").forEach((form) => {
      form.setAttribute("action", localizePath("search", locale));
      const input = form.querySelector("[data-search]");
      if (input) input.value = query;
    });
  }

  function applyCopy(locale) {
    const messages = translations[locale] || translations["en-GB"];
    document.documentElement.lang = locale;
    document.documentElement.dir = "ltr";
    document.querySelectorAll("[data-i18n]").forEach((node) => {
      const key = node.getAttribute("data-i18n");
      const value = messages.copy[key] || messages[key];
      if (value) node.textContent = value;
    });
    document.querySelectorAll("[data-i18n-nav]").forEach((node) => {
      node.textContent = messages.nav[node.getAttribute("data-i18n-nav")];
    });
    document.querySelectorAll("[data-i18n-control]").forEach((node) => {
      node.textContent = messages.controls[node.getAttribute("data-i18n-control")];
    });
    document.querySelectorAll("[data-i18n-aria-label]").forEach((node) => {
      const key = node.getAttribute("data-i18n-aria-label");
      const value = messages.controls[key];
      if (value) node.setAttribute("aria-label", value);
    });
    const brand = document.querySelector("[data-brand]");
    if (brand) brand.textContent = messages.brand;
    const search = document.querySelector("[data-search]");
    if (search) search.setAttribute("placeholder", messages.searchPlaceholder);
    const footer = document.querySelector("[data-footer]");
    if (footer) footer.textContent = messages.footer;
    document.title = messages.pageTitles[currentRoute()] || "Gitly";
  }

  function localizeApiValue(locale, key, field, value) {
    const messages = translations[locale] || translations["en-GB"];
    const copy = messages.copy;
    const normalized = `${key}.${field}`;
    const map = {
      "repo.visibility": copy["api.visibility.public"],
      "pulse.status": copy["api.status.active"],
      "workflows.primary_workflow": copy["api.workflow.primary"],
      "workflows.primary_cadence": copy["api.workflow.primaryCadence"]
    };
    return map[normalized] || value;
  }

  function applyTheme(theme) {
    const resolved = theme === "system"
      ? (window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light")
      : theme;
    document.documentElement.dataset.theme = resolved;
    document.querySelectorAll("[data-theme-option]").forEach((button) => {
      button.setAttribute("aria-pressed", button.getAttribute("data-theme-option") === theme ? "true" : "false");
    });
    localStorage.setItem("gitly-theme", theme);
  }

  async function hydrateApi(locale) {
    const targets = [
      ["/api/github/repository", "repo"],
      ["/api/github/pulls", "pulls"],
      ["/api/github/workflows", "workflows"],
      ["/api/github/pulse", "pulse"]
    ];
    for (const [url, key] of targets) {
      try {
        const response = await fetch(url, { headers: { Accept: "application/json" } });
        if (!response.ok) continue;
        const payload = await response.json();
        Object.entries(payload).forEach(([field, value]) => {
          document.querySelectorAll(`[data-api="${key}.${field}"]`).forEach((node) => {
            node.textContent = localizeApiValue(locale, key, field, value);
          });
        });
      } catch (_) {
        const flash = document.querySelector("[data-api-flash]");
        if (flash) {
          flash.hidden = false;
        }
      }
    }
  }

  function renderSearchResults(locale) {
    if (currentRoute() !== "search") return;
    const messages = translations[locale] || translations["en-GB"];
    const query = (new URLSearchParams(window.location.search).get("q") || "").trim().toLowerCase();
    const results = document.querySelector("[data-search-results]");
    const empty = document.querySelector("[data-search-empty]");
    const queryLabel = document.querySelector("[data-search-query]");
    if (!results || !empty || !queryLabel) return;
    queryLabel.textContent = query || "platform";

    const matches = searchIndex.filter((entry) => {
      if (!query) return true;
      return [entry.title, ...entry.terms].some((term) => term.toLowerCase().includes(query));
    });

    results.innerHTML = matches
      .map((entry) => {
        const href = localizePath(entry.route, locale);
        const type = messages.copy[`search.type.${entry.type}`] || entry.type;
        const summary = messages.copy[entry.summaryKey] || entry.summaryKey;
        return `<article class="summary-card search-result"><div class="meta-label">${type}</div><h2><a href="${href}">${entry.title}</a></h2><p>${summary}</p></article>`;
      })
      .join("");

    empty.hidden = matches.length !== 0;
  }

  function boot() {
    const locale = currentLocale();
    applyCopy(locale);
    applyLinks(locale);
    applySearchForms(locale);
    const storedTheme = localStorage.getItem("gitly-theme") || "system";
    applyTheme(storedTheme);
    document.querySelectorAll("[data-theme-option]").forEach((button) => {
      button.addEventListener("click", () => applyTheme(button.getAttribute("data-theme-option")));
    });
    renderSearchResults(locale);
    hydrateApi(locale);
  }

  return { boot };
})();

window.addEventListener("DOMContentLoaded", GITLY.boot);
