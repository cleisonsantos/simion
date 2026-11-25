//! Catálogo estático de emojis com busca offline (PT/EN), categorias e recentes.

use crate::store::fold;

/// Chave na tabela `settings` que persiste os últimos emojis usados.
pub const RECENT_EMOJIS_KEY: &str = "recent_emojis";

/// Quantidade máxima de emojis guardados em `Recentes`.
pub const RECENT_EMOJIS_LIMIT: usize = 8;

/// Categorias exibidas no seletor de ícone da página.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum EmojiCategory {
    #[default]
    All,
    Recents,
    Smileys,
    Animals,
    Food,
    Objects,
    Symbols,
}

impl EmojiCategory {
    pub const TABS: [Self; 7] = [
        Self::All,
        Self::Recents,
        Self::Smileys,
        Self::Animals,
        Self::Food,
        Self::Objects,
        Self::Symbols,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "Todos",
            Self::Recents => "Recentes",
            Self::Smileys => "Sorrisos",
            Self::Animals => "Animais",
            Self::Food => "Comida",
            Self::Objects => "Objetos",
            Self::Symbols => "Símbolos",
        }
    }
}

/// Um item do catálogo offline de emojis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EmojiEntry {
    pub emoji: &'static str,
    pub name: &'static str,
    pub category: EmojiCategory,
    /// Palavras-chave em português e inglês separadas por espaço.
    pub keywords: &'static str,
}

/// Os 48 emojis originais na ordem padrão seguidos pelo catálogo expandido.
///
/// Todos os glifos desta lista existem nas fontes embutidas (`NotoEmoji-Regular`
/// e `emoji-icon-font`) para evitar caixas vazias no seletor.
pub const CATALOG: &[EmojiEntry] = &[
    // --- Grade padrão (48 originais, na mesma ordem) ---
    EmojiEntry {
        emoji: "📄",
        name: "Página",
        category: EmojiCategory::Objects,
        keywords: "página documento folha texto nota page document file paper",
    },
    EmojiEntry {
        emoji: "📝",
        name: "Anotação",
        category: EmojiCategory::Objects,
        keywords: "anotação nota escrever lápis memorando rascunho memo note write pencil",
    },
    EmojiEntry {
        emoji: "📚",
        name: "Livros",
        category: EmojiCategory::Objects,
        keywords: "livros biblioteca leitura estudo estante books library study reading",
    },
    EmojiEntry {
        emoji: "📌",
        name: "Alfinete",
        category: EmojiCategory::Objects,
        keywords: "alfinete fixar pino marcador pin pushpin tack",
    },
    EmojiEntry {
        emoji: "💡",
        name: "Lâmpada",
        category: EmojiCategory::Objects,
        keywords: "lâmpada ideia dica luz inspiração lightbulb idea tip bright",
    },
    EmojiEntry {
        emoji: "✅",
        name: "Concluído",
        category: EmojiCategory::Symbols,
        keywords: "concluído feito certo tarefa aprovado check done ok checkmark",
    },
    EmojiEntry {
        emoji: "🚀",
        name: "Foguete",
        category: EmojiCategory::Objects,
        keywords: "foguete lançamento rápido espaço nave rocket launch fast space ship",
    },
    EmojiEntry {
        emoji: "⭐",
        name: "Estrela",
        category: EmojiCategory::Symbols,
        keywords: "estrela favorito destaque nota star favorite rating",
    },
    EmojiEntry {
        emoji: "❤",
        name: "Coração",
        category: EmojiCategory::Symbols,
        keywords: "coração amor carinho paixão vermelho heart love red",
    },
    EmojiEntry {
        emoji: "🎯",
        name: "Alvo",
        category: EmojiCategory::Objects,
        keywords: "alvo meta objetivo foco dardo target goal focus bullseye dart",
    },
    EmojiEntry {
        emoji: "🔥",
        name: "Fogo",
        category: EmojiCategory::Symbols,
        keywords: "fogo chama quente incêndio destaque tendência fire flame hot lit burn",
    },
    EmojiEntry {
        emoji: "🌱",
        name: "Broto",
        category: EmojiCategory::Animals,
        keywords: "broto planta muda natureza crescer jardim seedling sprout plant nature grow",
    },
    EmojiEntry {
        emoji: "🍀",
        name: "Trevo",
        category: EmojiCategory::Animals,
        keywords: "trevo sorte folha quatro folhas natureza clover luck four leaf nature",
    },
    EmojiEntry {
        emoji: "🌊",
        name: "Onda",
        category: EmojiCategory::Animals,
        keywords: "onda mar oceano água praia surfe wave ocean sea water beach",
    },
    EmojiEntry {
        emoji: "☀",
        name: "Sol",
        category: EmojiCategory::Symbols,
        keywords: "sol dia verão calor ensolarado luz sun sunny day summer bright",
    },
    EmojiEntry {
        emoji: "🌙",
        name: "Lua",
        category: EmojiCategory::Symbols,
        keywords: "lua noite crescente sono escuro moon night crescent sleep dark",
    },
    EmojiEntry {
        emoji: "🎵",
        name: "Nota musical",
        category: EmojiCategory::Symbols,
        keywords: "música nota som melodia canção áudio music note song sound audio",
    },
    EmojiEntry {
        emoji: "🎨",
        name: "Paleta",
        category: EmojiCategory::Objects,
        keywords: "paleta arte pintura cor desenho design criativo palette art paint color draw",
    },
    EmojiEntry {
        emoji: "🏠",
        name: "Casa",
        category: EmojiCategory::Objects,
        keywords: "casa lar residência moradia início home house building",
    },
    EmojiEntry {
        emoji: "💼",
        name: "Maleta",
        category: EmojiCategory::Objects,
        keywords: "maleta trabalho negócios escritório carreira emprego briefcase work business office job",
    },
    EmojiEntry {
        emoji: "📅",
        name: "Calendário",
        category: EmojiCategory::Objects,
        keywords: "calendário data agenda evento dia cronograma calendar date schedule event",
    },
    EmojiEntry {
        emoji: "🔍",
        name: "Lupa",
        category: EmojiCategory::Objects,
        keywords: "lupa busca procurar pesquisar investigar zoom search magnifying glass find",
    },
    EmojiEntry {
        emoji: "⚙",
        name: "Engrenagem",
        category: EmojiCategory::Objects,
        keywords: "engrenagem configuração ajuste sistema mecânica gear settings config cog",
    },
    EmojiEntry {
        emoji: "🔒",
        name: "Cadeado",
        category: EmojiCategory::Objects,
        keywords: "cadeado trancado segurança privado senha fechado lock security private password",
    },
    EmojiEntry {
        emoji: "🎁",
        name: "Presente",
        category: EmojiCategory::Objects,
        keywords: "presente caixa aniversário surpresa pacote gift present birthday box",
    },
    EmojiEntry {
        emoji: "🐱",
        name: "Gato",
        category: EmojiCategory::Animals,
        keywords: "gato gatinho felino pet animal bichano cat kitty pet animal",
    },
    EmojiEntry {
        emoji: "🐶",
        name: "Cachorro",
        category: EmojiCategory::Animals,
        keywords: "cachorro cão filhote pet animal dog puppy pet animal",
    },
    EmojiEntry {
        emoji: "🍎",
        name: "Maçã",
        category: EmojiCategory::Food,
        keywords: "maçã fruta vermelha comida saudável apple fruit red food healthy",
    },
    EmojiEntry {
        emoji: "🌻",
        name: "Girassol",
        category: EmojiCategory::Animals,
        keywords: "girassol flor planta amarelo verão natureza sunflower flower plant yellow nature",
    },
    EmojiEntry {
        emoji: "🌈",
        name: "Arco-íris",
        category: EmojiCategory::Symbols,
        keywords: "arco-íris cores céu chuva orgulho natureza rainbow colors sky pride",
    },
    EmojiEntry {
        emoji: "☕",
        name: "Café",
        category: EmojiCategory::Food,
        keywords: "café chá xícara bebida quente manhã pausa coffee tea cup drink hot morning",
    },
    EmojiEntry {
        emoji: "🏆",
        name: "Troféu",
        category: EmojiCategory::Objects,
        keywords: "troféu prêmio vitória campeão conquista copa trophy award win champion prize",
    },
    EmojiEntry {
        emoji: "📷",
        name: "Câmera",
        category: EmojiCategory::Objects,
        keywords: "câmera foto fotografia imagem registro camera photo picture photography",
    },
    EmojiEntry {
        emoji: "✏",
        name: "Lápis",
        category: EmojiCategory::Objects,
        keywords: "lápis escrever editar desenho rascunho pencil write edit draw",
    },
    EmojiEntry {
        emoji: "📎",
        name: "Clipe",
        category: EmojiCategory::Objects,
        keywords: "clipe anexo papel escritório paperclip attachment clip office",
    },
    EmojiEntry {
        emoji: "🎓",
        name: "Formatura",
        category: EmojiCategory::Objects,
        keywords: "formatura capelo universidade estudo educação curso graduation cap university study education",
    },
    EmojiEntry {
        emoji: "💬",
        name: "Balão de fala",
        category: EmojiCategory::Symbols,
        keywords: "conversa fala mensagem comentário bate-papo chat speech bubble talk message comment",
    },
    EmojiEntry {
        emoji: "🛠",
        name: "Ferramentas",
        category: EmojiCategory::Objects,
        keywords: "ferramentas martelo chave conserto manutenção construir tools hammer wrench fix build",
    },
    EmojiEntry {
        emoji: "🎉",
        name: "Festa",
        category: EmojiCategory::Objects,
        keywords: "festa comemoração parabéns confete celebração party celebration confetti tada",
    },
    EmojiEntry {
        emoji: "🔬",
        name: "Microscópio",
        category: EmojiCategory::Objects,
        keywords: "microscópio ciência laboratório pesquisa biologia microscope science lab research",
    },
    EmojiEntry {
        emoji: "🔖",
        name: "Marcador",
        category: EmojiCategory::Objects,
        keywords: "marcador etiqueta favorito salvar página bookmark tag label save",
    },
    EmojiEntry {
        emoji: "📊",
        name: "Gráfico",
        category: EmojiCategory::Objects,
        keywords: "gráfico barras dados estatística relatório métrica chart bar graph data stats report",
    },
    EmojiEntry {
        emoji: "🗺",
        name: "Mapa",
        category: EmojiCategory::Objects,
        keywords: "mapa mundo viagem geografia rota turismo map world travel geography",
    },
    EmojiEntry {
        emoji: "⏰",
        name: "Despertador",
        category: EmojiCategory::Objects,
        keywords: "despertador relógio alarme hora tempo prazo alarm clock time deadline",
    },
    EmojiEntry {
        emoji: "💎",
        name: "Diamante",
        category: EmojiCategory::Objects,
        keywords: "diamante joia gema pedra preciosa cristal gem diamond jewel crystal",
    },
    EmojiEntry {
        emoji: "🐢",
        name: "Tartaruga",
        category: EmojiCategory::Animals,
        keywords: "tartaruga réptil lento animal casco natureza turtle tortoise slow reptile animal",
    },
    EmojiEntry {
        emoji: "🌸",
        name: "Flor de cerejeira",
        category: EmojiCategory::Animals,
        keywords: "flor cerejeira sakura primavera rosa natureza flower blossom cherry sakura spring pink",
    },
    EmojiEntry {
        emoji: "🍕",
        name: "Pizza",
        category: EmojiCategory::Food,
        keywords: "pizza fatia queijo massa comida lanche slice cheese food fastfood",
    },
    // --- Sorrisos e expressões ---
    EmojiEntry {
        emoji: "😀",
        name: "Sorriso",
        category: EmojiCategory::Smileys,
        keywords: "sorriso feliz alegre rosto carinha grin happy smile face joy",
    },
    EmojiEntry {
        emoji: "😁",
        name: "Sorriso largo",
        category: EmojiCategory::Smileys,
        keywords: "sorriso dentes feliz radiante beaming grin happy smile",
    },
    EmojiEntry {
        emoji: "😂",
        name: "Chorando de rir",
        category: EmojiCategory::Smileys,
        keywords: "rir risada gargalhada engraçado lágrimas joy laugh funny lol tears",
    },
    EmojiEntry {
        emoji: "😄",
        name: "Sorridente",
        category: EmojiCategory::Smileys,
        keywords: "sorridente feliz alegria contente smile happy laugh glad",
    },
    EmojiEntry {
        emoji: "😅",
        name: "Suor frio",
        category: EmojiCategory::Smileys,
        keywords: "suor alívio nervoso riso sem graça sweat smile nervous relief phew",
    },
    EmojiEntry {
        emoji: "😉",
        name: "Piscadinha",
        category: EmojiCategory::Smileys,
        keywords: "piscadinha piscar cúmplice brincadeira wink face playful",
    },
    EmojiEntry {
        emoji: "😊",
        name: "Sorriso tímido",
        category: EmojiCategory::Smileys,
        keywords: "sorriso tímido fofo gentil feliz blush warm smile cute happy",
    },
    EmojiEntry {
        emoji: "😋",
        name: "Delícia",
        category: EmojiCategory::Smileys,
        keywords: "delícia gostoso língua fome saboroso yum delicious tasty tongue",
    },
    EmojiEntry {
        emoji: "😌",
        name: "Aliviado",
        category: EmojiCategory::Smileys,
        keywords: "aliviado calmo paz tranquilo sereno relieved calm peaceful serene",
    },
    EmojiEntry {
        emoji: "😍",
        name: "Apaixonado",
        category: EmojiCategory::Smileys,
        keywords: "apaixonado amor olhos coração adorar heart eyes love crush adore",
    },
    EmojiEntry {
        emoji: "😎",
        name: "Óculos escuros",
        category: EmojiCategory::Smileys,
        keywords: "legal estiloso óculos escuros descolado confiante cool sunglasses awesome",
    },
    EmojiEntry {
        emoji: "😏",
        name: "Sorriso maroto",
        category: EmojiCategory::Smileys,
        keywords: "maroto malandro irônico smirk sly cheeky",
    },
    EmojiEntry {
        emoji: "😇",
        name: "Anjinho",
        category: EmojiCategory::Smileys,
        keywords: "anjinho auréola inocente santo bom angel halo innocent blessed",
    },
    EmojiEntry {
        emoji: "😘",
        name: "Beijo",
        category: EmojiCategory::Smileys,
        keywords: "beijo beijinho carinho amor coração kiss blowing love affection",
    },
    EmojiEntry {
        emoji: "😜",
        name: "Brincalhão",
        category: EmojiCategory::Smileys,
        keywords: "brincalhão língua piscando divertido doido wink tongue playful crazy",
    },
    EmojiEntry {
        emoji: "😐",
        name: "Neutro",
        category: EmojiCategory::Smileys,
        keywords: "neutro sério indiferente sem expressão neutral meh poker face",
    },
    EmojiEntry {
        emoji: "😒",
        name: "Entediado",
        category: EmojiCategory::Smileys,
        keywords: "entediado chateado desconfiado cansado unamused bored annoyed",
    },
    EmojiEntry {
        emoji: "😔",
        name: "Pensativo",
        category: EmojiCategory::Smileys,
        keywords: "pensativo triste cabisbaixo desanimado pensive sad down",
    },
    EmojiEntry {
        emoji: "😕",
        name: "Confuso",
        category: EmojiCategory::Smileys,
        keywords: "confuso dúvida incerto perplexo confused unsure puzzled",
    },
    EmojiEntry {
        emoji: "😢",
        name: "Chorando",
        category: EmojiCategory::Smileys,
        keywords: "chorando lágrima triste mágoa cry tear sad",
    },
    EmojiEntry {
        emoji: "😭",
        name: "Choro intenso",
        category: EmojiCategory::Smileys,
        keywords: "choro chorando muito triste desespero sob crying loud sad",
    },
    EmojiEntry {
        emoji: "😤",
        name: "Bufando",
        category: EmojiCategory::Smileys,
        keywords: "bufando bravo determinado frustrado triunfo huff angry triumph steam",
    },
    EmojiEntry {
        emoji: "😡",
        name: "Furioso",
        category: EmojiCategory::Smileys,
        keywords: "furioso raiva bravo irritado vermelho angry mad rage furious",
    },
    EmojiEntry {
        emoji: "😱",
        name: "Assustado",
        category: EmojiCategory::Smileys,
        keywords: "assustado grito medo choque susto pânico scream fear shock scared",
    },
    EmojiEntry {
        emoji: "😳",
        name: "Corado",
        category: EmojiCategory::Smileys,
        keywords: "corado surpreso vergonha constrangido flushed embarrassed surprised",
    },
    EmojiEntry {
        emoji: "😴",
        name: "Dormindo",
        category: EmojiCategory::Smileys,
        keywords: "dormindo sono cochilo cansado zzz sleeping sleep tired nap",
    },
    EmojiEntry {
        emoji: "😵",
        name: "Tonto",
        category: EmojiCategory::Smileys,
        keywords: "tonto zonzo confuso atordoado dizzy dazed stunned",
    },
    EmojiEntry {
        emoji: "😷",
        name: "Máscara",
        category: EmojiCategory::Smileys,
        keywords: "máscara doente saúde médico resfriado mask sick health doctor",
    },
    EmojiEntry {
        emoji: "😈",
        name: "Diabinho",
        category: EmojiCategory::Smileys,
        keywords: "diabinho travesso malicioso chifres devil imp mischievous",
    },
    EmojiEntry {
        emoji: "👻",
        name: "Fantasma",
        category: EmojiCategory::Smileys,
        keywords: "fantasma assombração halloween susto ghost spooky halloween boo",
    },
    EmojiEntry {
        emoji: "👽",
        name: "Alienígena",
        category: EmojiCategory::Smileys,
        keywords: "alienígena et espaço ovni extraterrestre alien ufo space",
    },
    EmojiEntry {
        emoji: "👾",
        name: "Monstro pixel",
        category: EmojiCategory::Smileys,
        keywords: "monstro jogo retro arcade game alien monster pixel",
    },
    EmojiEntry {
        emoji: "💀",
        name: "Caveira",
        category: EmojiCategory::Smileys,
        keywords: "caveira crânio morte perigo halloween skull death danger skeleton",
    },
    EmojiEntry {
        emoji: "💩",
        name: "Cocô",
        category: EmojiCategory::Smileys,
        keywords: "cocô engraçado brincadeira poop poo funny",
    },
    EmojiEntry {
        emoji: "🙈",
        name: "Macaco não vê",
        category: EmojiCategory::Smileys,
        keywords: "macaco vergonha esconder olhos monkey see no evil shy",
    },
    EmojiEntry {
        emoji: "🙉",
        name: "Macaco não ouve",
        category: EmojiCategory::Smileys,
        keywords: "macaco ouvidos surdo ignorar monkey hear no evil",
    },
    EmojiEntry {
        emoji: "🙊",
        name: "Macaco não fala",
        category: EmojiCategory::Smileys,
        keywords: "macaco segredo silêncio boca monkey speak no evil secret oops",
    },
    EmojiEntry {
        emoji: "👋",
        name: "Aceno",
        category: EmojiCategory::Smileys,
        keywords: "aceno olá oi tchau mão cumprimento wave hello hi bye hand",
    },
    EmojiEntry {
        emoji: "👍",
        name: "Joinha",
        category: EmojiCategory::Smileys,
        keywords: "joinha curtir legal aprovado sim positivo thumbs up like ok yes good",
    },
    EmojiEntry {
        emoji: "👎",
        name: "Negativo",
        category: EmojiCategory::Smileys,
        keywords: "negativo não reprovado ruim descurtir thumbs down dislike no bad",
    },
    EmojiEntry {
        emoji: "👏",
        name: "Palmas",
        category: EmojiCategory::Smileys,
        keywords: "palmas aplausos parabéns bravo clap applause congrats bravo",
    },
    EmojiEntry {
        emoji: "🙌",
        name: "Mãos para cima",
        category: EmojiCategory::Smileys,
        keywords: "comemoração viva mãos erguidas sucesso hooray celebrate hands praise",
    },
    EmojiEntry {
        emoji: "🙏",
        name: "Obrigado",
        category: EmojiCategory::Smileys,
        keywords: "obrigado por favor oração gratidão mãos juntas pray thanks please grateful",
    },
    EmojiEntry {
        emoji: "💪",
        name: "Força",
        category: EmojiCategory::Smileys,
        keywords: "força músculo treino forte academia determinação muscle strong flex workout gym",
    },
    EmojiEntry {
        emoji: "✌",
        name: "Paz e amor",
        category: EmojiCategory::Smileys,
        keywords: "paz vitória dedos mão peace victory hand v",
    },
    EmojiEntry {
        emoji: "👌",
        name: "Perfeito",
        category: EmojiCategory::Smileys,
        keywords: "perfeito ok certo combinado sinal ok hand perfect nice",
    },
    EmojiEntry {
        emoji: "👀",
        name: "Olhos",
        category: EmojiCategory::Smileys,
        keywords: "olhos olhar observar atenção espiar eyes look watch see",
    },
    EmojiEntry {
        emoji: "👑",
        name: "Coroa",
        category: EmojiCategory::Smileys,
        keywords: "coroa rei rainha realeza líder topo crown king queen royal leader",
    },
    // --- Animais e natureza ---
    EmojiEntry {
        emoji: "🐭",
        name: "Rato",
        category: EmojiCategory::Animals,
        keywords: "rato ratinho roedor animal mouse rat rodent",
    },
    EmojiEntry {
        emoji: "🐹",
        name: "Hamster",
        category: EmojiCategory::Animals,
        keywords: "hamster roedor pet fofo animal hamster pet cute",
    },
    EmojiEntry {
        emoji: "🐰",
        name: "Coelho",
        category: EmojiCategory::Animals,
        keywords: "coelho coelhinho páscoa animal rabbit bunny easter",
    },
    EmojiEntry {
        emoji: "🐻",
        name: "Urso",
        category: EmojiCategory::Animals,
        keywords: "urso ursinho floresta animal bear forest animal",
    },
    EmojiEntry {
        emoji: "🐼",
        name: "Panda",
        category: EmojiCategory::Animals,
        keywords: "panda urso bambu fofo animal panda bear bamboo",
    },
    EmojiEntry {
        emoji: "🐨",
        name: "Coala",
        category: EmojiCategory::Animals,
        keywords: "coala austrália fofo animal koala australia cute",
    },
    EmojiEntry {
        emoji: "🐯",
        name: "Tigre",
        category: EmojiCategory::Animals,
        keywords: "tigre felino selva forte animal tiger cat jungle wild",
    },
    EmojiEntry {
        emoji: "🐮",
        name: "Vaca",
        category: EmojiCategory::Animals,
        keywords: "vaca fazenda leite animal cow farm milk",
    },
    EmojiEntry {
        emoji: "🐷",
        name: "Porco",
        category: EmojiCategory::Animals,
        keywords: "porco porquinho fazenda animal pig piggy farm",
    },
    EmojiEntry {
        emoji: "🐸",
        name: "Sapo",
        category: EmojiCategory::Animals,
        keywords: "sapo rã lagoa verde animal frog toad green pond",
    },
    EmojiEntry {
        emoji: "🐵",
        name: "Macaco",
        category: EmojiCategory::Animals,
        keywords: "macaco primata selva animal monkey ape jungle",
    },
    EmojiEntry {
        emoji: "🐺",
        name: "Lobo",
        category: EmojiCategory::Animals,
        keywords: "lobo selvagem matilha animal wolf wild howl",
    },
    EmojiEntry {
        emoji: "🐴",
        name: "Cavalo",
        category: EmojiCategory::Animals,
        keywords: "cavalo equino fazenda corrida animal horse pony farm",
    },
    EmojiEntry {
        emoji: "🐔",
        name: "Galinha",
        category: EmojiCategory::Animals,
        keywords: "galinha frango ave fazenda animal chicken hen bird farm",
    },
    EmojiEntry {
        emoji: "🐧",
        name: "Pinguim",
        category: EmojiCategory::Animals,
        keywords: "pinguim gelo antártida linux ave animal penguin ice bird linux",
    },
    EmojiEntry {
        emoji: "🐦",
        name: "Pássaro",
        category: EmojiCategory::Animals,
        keywords: "pássaro ave voar céu natureza bird fly sky nature",
    },
    EmojiEntry {
        emoji: "🐣",
        name: "Pintinho",
        category: EmojiCategory::Animals,
        keywords: "pintinho ovo nascer novo começo chick hatch egg newborn",
    },
    EmojiEntry {
        emoji: "🐍",
        name: "Cobra",
        category: EmojiCategory::Animals,
        keywords: "cobra serpente réptil python animal snake serpent python reptile",
    },
    EmojiEntry {
        emoji: "🐊",
        name: "Crocodilo",
        category: EmojiCategory::Animals,
        keywords: "crocodilo jacaré réptil animal crocodile alligator reptile",
    },
    EmojiEntry {
        emoji: "🐉",
        name: "Dragão",
        category: EmojiCategory::Animals,
        keywords: "dragão fantasia fogo lenda mítico dragon fantasy legend mythical",
    },
    EmojiEntry {
        emoji: "🐳",
        name: "Baleia",
        category: EmojiCategory::Animals,
        keywords: "baleia oceano mar docker animal whale ocean sea docker",
    },
    EmojiEntry {
        emoji: "🐬",
        name: "Golfinho",
        category: EmojiCategory::Animals,
        keywords: "golfinho mar oceano inteligente animal dolphin ocean sea",
    },
    EmojiEntry {
        emoji: "🐟",
        name: "Peixe",
        category: EmojiCategory::Animals,
        keywords: "peixe mar rio aquário animal fish sea ocean river",
    },
    EmojiEntry {
        emoji: "🐙",
        name: "Polvo",
        category: EmojiCategory::Animals,
        keywords: "polvo tentáculos mar oceano animal octopus ocean sea",
    },
    EmojiEntry {
        emoji: "🐝",
        name: "Abelha",
        category: EmojiCategory::Animals,
        keywords: "abelha mel inseto natureza colmeia bee honey insect nature",
    },
    EmojiEntry {
        emoji: "🐞",
        name: "Joaninha",
        category: EmojiCategory::Animals,
        keywords: "joaninha inseto sorte bug vermelho ladybug bug beetle luck",
    },
    EmojiEntry {
        emoji: "🐛",
        name: "Lagarta",
        category: EmojiCategory::Animals,
        keywords: "lagarta inseto bug erro software bug caterpillar insect",
    },
    EmojiEntry {
        emoji: "🐌",
        name: "Caracol",
        category: EmojiCategory::Animals,
        keywords: "caracol lesma lento concha animal snail slow shell",
    },
    EmojiEntry {
        emoji: "🐘",
        name: "Elefante",
        category: EmojiCategory::Animals,
        keywords: "elefante grande memória savana animal elephant big safari",
    },
    EmojiEntry {
        emoji: "🐾",
        name: "Pegadas",
        category: EmojiCategory::Animals,
        keywords: "pegadas pata animal rastro pet paw prints animal track pet",
    },
    EmojiEntry {
        emoji: "🌹",
        name: "Rosa",
        category: EmojiCategory::Animals,
        keywords: "rosa flor romântico amor vermelha rose flower red romance",
    },
    EmojiEntry {
        emoji: "🌷",
        name: "Tulipa",
        category: EmojiCategory::Animals,
        keywords: "tulipa flor primavera jardim natureza tulip flower spring garden",
    },
    EmojiEntry {
        emoji: "🌺",
        name: "Hibisco",
        category: EmojiCategory::Animals,
        keywords: "hibisco flor tropical verão natureza hibiscus flower tropical",
    },
    EmojiEntry {
        emoji: "🌼",
        name: "Margarida",
        category: EmojiCategory::Animals,
        keywords: "margarida flor campo primavera natureza blossom daisy flower",
    },
    EmojiEntry {
        emoji: "💐",
        name: "Buquê",
        category: EmojiCategory::Animals,
        keywords: "buquê flores presente jardim bouquet flowers gift",
    },
    EmojiEntry {
        emoji: "🌲",
        name: "Pinheiro",
        category: EmojiCategory::Animals,
        keywords: "pinheiro árvore floresta natureza verde evergreen tree forest nature",
    },
    EmojiEntry {
        emoji: "🌳",
        name: "Árvore",
        category: EmojiCategory::Animals,
        keywords: "árvore floresta parque sombra natureza tree forest nature green",
    },
    EmojiEntry {
        emoji: "🌴",
        name: "Palmeira",
        category: EmojiCategory::Animals,
        keywords: "palmeira coqueiro praia férias verão tropical palm tree beach vacation",
    },
    EmojiEntry {
        emoji: "🌵",
        name: "Cacto",
        category: EmojiCategory::Animals,
        keywords: "cacto deserto planta suculenta natureza cactus desert plant",
    },
    EmojiEntry {
        emoji: "🌿",
        name: "Erva",
        category: EmojiCategory::Animals,
        keywords: "erva folha verde planta chá natureza herb leaf green plant",
    },
    EmojiEntry {
        emoji: "🍁",
        name: "Folha de bordo",
        category: EmojiCategory::Animals,
        keywords: "folha outono bordo canadá natureza maple leaf autumn fall",
    },
    EmojiEntry {
        emoji: "🍄",
        name: "Cogumelo",
        category: EmojiCategory::Animals,
        keywords: "cogumelo fungo floresta natureza mushroom fungus forest",
    },
    EmojiEntry {
        emoji: "🌍",
        name: "Planeta Terra",
        category: EmojiCategory::Animals,
        keywords: "terra planeta mundo globo natureza earth globe world planet",
    },
    EmojiEntry {
        emoji: "🌋",
        name: "Vulcão",
        category: EmojiCategory::Animals,
        keywords: "vulcão lava erupção montanha fogo volcano lava mountain fire",
    },
    // --- Comida e bebida ---
    EmojiEntry {
        emoji: "🍏",
        name: "Maçã verde",
        category: EmojiCategory::Food,
        keywords: "maçã verde fruta saudável comida green apple fruit healthy",
    },
    EmojiEntry {
        emoji: "🍊",
        name: "Laranja",
        category: EmojiCategory::Food,
        keywords: "laranja tangerina mexerica fruta cítrica orange tangerine citrus fruit",
    },
    EmojiEntry {
        emoji: "🍋",
        name: "Limão",
        category: EmojiCategory::Food,
        keywords: "limão fruta azedo cítrico amarelo lemon citrus sour yellow fruit",
    },
    EmojiEntry {
        emoji: "🍌",
        name: "Banana",
        category: EmojiCategory::Food,
        keywords: "banana fruta amarela tropical comida banana fruit yellow",
    },
    EmojiEntry {
        emoji: "🍉",
        name: "Melancia",
        category: EmojiCategory::Food,
        keywords: "melancia fruta verão refrescante comida watermelon fruit summer",
    },
    EmojiEntry {
        emoji: "🍇",
        name: "Uva",
        category: EmojiCategory::Food,
        keywords: "uva uvas fruta vinho roxo grapes fruit wine purple",
    },
    EmojiEntry {
        emoji: "🍓",
        name: "Morango",
        category: EmojiCategory::Food,
        keywords: "morango fruta vermelho doce sobremesa strawberry fruit red sweet",
    },
    EmojiEntry {
        emoji: "🍒",
        name: "Cereja",
        category: EmojiCategory::Food,
        keywords: "cereja fruta vermelho doce cherry fruit red sweet",
    },
    EmojiEntry {
        emoji: "🍑",
        name: "Pêssego",
        category: EmojiCategory::Food,
        keywords: "pêssego fruta doce verão peach fruit sweet",
    },
    EmojiEntry {
        emoji: "🍍",
        name: "Abacaxi",
        category: EmojiCategory::Food,
        keywords: "abacaxi fruta tropical amarelo verão pineapple fruit tropical",
    },
    EmojiEntry {
        emoji: "🍅",
        name: "Tomate",
        category: EmojiCategory::Food,
        keywords: "tomate salada pomodoro legume fruta tomato vegetable pomodoro",
    },
    EmojiEntry {
        emoji: "🌽",
        name: "Milho",
        category: EmojiCategory::Food,
        keywords: "milho espiga fazenda festa junina comida corn maize farm",
    },
    EmojiEntry {
        emoji: "🍞",
        name: "Pão",
        category: EmojiCategory::Food,
        keywords: "pão padaria café da manhã trigo comida bread bakery breakfast",
    },
    EmojiEntry {
        emoji: "🍔",
        name: "Hambúrguer",
        category: EmojiCategory::Food,
        keywords: "hambúrguer lanche sanduíche carne comida burger hamburger sandwich fastfood",
    },
    EmojiEntry {
        emoji: "🍟",
        name: "Batata frita",
        category: EmojiCategory::Food,
        keywords: "batata frita lanche comida fries french fries chips snack",
    },
    EmojiEntry {
        emoji: "🍝",
        name: "Macarrão",
        category: EmojiCategory::Food,
        keywords: "macarrão espaguete massa almoço jantar pasta spaghetti noodles",
    },
    EmojiEntry {
        emoji: "🍜",
        name: "Lámen",
        category: EmojiCategory::Food,
        keywords: "lámen sopa macarrão tigela japonesa comida ramen noodles soup bowl",
    },
    EmojiEntry {
        emoji: "🍣",
        name: "Sushi",
        category: EmojiCategory::Food,
        keywords: "sushi japonesa peixe arroz comida sushi japanese fish rice",
    },
    EmojiEntry {
        emoji: "🍛",
        name: "Curry",
        category: EmojiCategory::Food,
        keywords: "curry arroz prato temperado almoço curry rice meal",
    },
    EmojiEntry {
        emoji: "🍚",
        name: "Arroz",
        category: EmojiCategory::Food,
        keywords: "arroz tigela refeição almoço comida rice bowl meal",
    },
    EmojiEntry {
        emoji: "🍳",
        name: "Ovo frito",
        category: EmojiCategory::Food,
        keywords: "ovo frito frigideira cozinha café da manhã egg cooking breakfast fry",
    },
    EmojiEntry {
        emoji: "🍗",
        name: "Frango",
        category: EmojiCategory::Food,
        keywords: "frango coxa carne churrasco comida chicken poultry meat",
    },
    EmojiEntry {
        emoji: "🍲",
        name: "Panela",
        category: EmojiCategory::Food,
        keywords: "panela ensopado sopa receita cozinha comida stew pot soup recipe",
    },
    EmojiEntry {
        emoji: "🍱",
        name: "Marmita",
        category: EmojiCategory::Food,
        keywords: "marmita bentô almoço refeição comida bento lunchbox meal",
    },
    EmojiEntry {
        emoji: "🍰",
        name: "Fatia de bolo",
        category: EmojiCategory::Food,
        keywords: "bolo fatia sobremesa doce festa cake shortcake dessert sweet",
    },
    EmojiEntry {
        emoji: "🎂",
        name: "Bolo de aniversário",
        category: EmojiCategory::Food,
        keywords: "bolo aniversário velas parabéns festa birthday cake candles party",
    },
    EmojiEntry {
        emoji: "🍪",
        name: "Biscoito",
        category: EmojiCategory::Food,
        keywords: "biscoito bolacha cookie doce chocolate lanche cookie biscuit sweet snack",
    },
    EmojiEntry {
        emoji: "🍩",
        name: "Rosquinha",
        category: EmojiCategory::Food,
        keywords: "rosquinha donut doce sobremesa café donut doughnut sweet dessert",
    },
    EmojiEntry {
        emoji: "🍫",
        name: "Chocolate",
        category: EmojiCategory::Food,
        keywords: "chocolate barra doce sobremesa cacau chocolate bar sweet candy",
    },
    EmojiEntry {
        emoji: "🍦",
        name: "Sorvete",
        category: EmojiCategory::Food,
        keywords: "sorvete casquinha gelado verão sobremesa ice cream cone dessert summer",
    },
    EmojiEntry {
        emoji: "🍯",
        name: "Mel",
        category: EmojiCategory::Food,
        keywords: "mel pote abelha doce natural honey pot sweet",
    },
    EmojiEntry {
        emoji: "🍵",
        name: "Chá verde",
        category: EmojiCategory::Food,
        keywords: "chá verde xícara matcha bebida quente calma tea green matcha cup drink",
    },
    EmojiEntry {
        emoji: "🍺",
        name: "Cerveja",
        category: EmojiCategory::Food,
        keywords: "cerveja chope caneca bar bebida beer mug drink pub",
    },
    EmojiEntry {
        emoji: "🍻",
        name: "Brinde",
        category: EmojiCategory::Food,
        keywords: "brinde cervejas festa amigos comemoração cheers beers celebration",
    },
    EmojiEntry {
        emoji: "🍷",
        name: "Vinho",
        category: EmojiCategory::Food,
        keywords: "vinho taça jantar bebida festa wine glass drink dinner",
    },
    EmojiEntry {
        emoji: "🍹",
        name: "Drinque tropical",
        category: EmojiCategory::Food,
        keywords: "drinque coquetel suco praia férias bebida tropical drink cocktail juice",
    },
    EmojiEntry {
        emoji: "🍴",
        name: "Talheres",
        category: EmojiCategory::Food,
        keywords: "talheres garfo faca restaurante refeição receita fork knife restaurant meal",
    },
    // --- Objetos, lugares e atividades ---
    EmojiEntry {
        emoji: "📁",
        name: "Pasta",
        category: EmojiCategory::Objects,
        keywords: "pasta diretório arquivo organizar documentos folder directory file",
    },
    EmojiEntry {
        emoji: "📂",
        name: "Pasta aberta",
        category: EmojiCategory::Objects,
        keywords: "pasta aberta diretório arquivos organizar open folder directory",
    },
    EmojiEntry {
        emoji: "📋",
        name: "Prancheta",
        category: EmojiCategory::Objects,
        keywords: "prancheta lista tarefas checklist relatório clipboard list tasks checklist",
    },
    EmojiEntry {
        emoji: "📓",
        name: "Caderno",
        category: EmojiCategory::Objects,
        keywords: "caderno diário notas estudo livro notebook journal diary notes",
    },
    EmojiEntry {
        emoji: "📖",
        name: "Livro aberto",
        category: EmojiCategory::Objects,
        keywords: "livro aberto leitura estudo conhecimento guia open book reading study guide",
    },
    EmojiEntry {
        emoji: "📒",
        name: "Caderno amarelo",
        category: EmojiCategory::Objects,
        keywords: "caderno contabilidade finanças notas ledger notebook finance",
    },
    EmojiEntry {
        emoji: "📜",
        name: "Pergaminho",
        category: EmojiCategory::Objects,
        keywords: "pergaminho história documento antigo texto scroll ancient document history",
    },
    EmojiEntry {
        emoji: "📰",
        name: "Jornal",
        category: EmojiCategory::Objects,
        keywords: "jornal notícias artigo imprensa leitura newspaper news article press",
    },
    EmojiEntry {
        emoji: "📈",
        name: "Gráfico subindo",
        category: EmojiCategory::Objects,
        keywords: "gráfico crescimento alta lucro sucesso progresso chart up growth profit trend",
    },
    EmojiEntry {
        emoji: "📉",
        name: "Gráfico descendo",
        category: EmojiCategory::Objects,
        keywords: "gráfico queda redução baixa análise chart down decrease drop",
    },
    EmojiEntry {
        emoji: "💻",
        name: "Notebook",
        category: EmojiCategory::Objects,
        keywords: "notebook computador laptop programação código tecnologia laptop computer code dev tech",
    },
    EmojiEntry {
        emoji: "📱",
        name: "Celular",
        category: EmojiCategory::Objects,
        keywords: "celular telefone smartphone aplicativo mobile phone smartphone mobile app",
    },
    EmojiEntry {
        emoji: "💾",
        name: "Disquete",
        category: EmojiCategory::Objects,
        keywords: "disquete salvar backup armazenamento arquivo floppy disk save backup storage",
    },
    EmojiEntry {
        emoji: "🔋",
        name: "Bateria",
        category: EmojiCategory::Objects,
        keywords: "bateria energia carga força battery power energy charge",
    },
    EmojiEntry {
        emoji: "🔌",
        name: "Tomada",
        category: EmojiCategory::Objects,
        keywords: "tomada plugue conexão energia plugin integração plug electric power connect",
    },
    EmojiEntry {
        emoji: "🔑",
        name: "Chave",
        category: EmojiCategory::Objects,
        keywords: "chave acesso senha segredo solução segurança key access password secret",
    },
    EmojiEntry {
        emoji: "🔓",
        name: "Cadeado aberto",
        category: EmojiCategory::Objects,
        keywords: "cadeado aberto destrancado acesso livre unlock open access",
    },
    EmojiEntry {
        emoji: "🔧",
        name: "Chave inglesa",
        category: EmojiCategory::Objects,
        keywords: "chave inglesa ferramenta conserto ajuste manutenção wrench tool fix repair",
    },
    EmojiEntry {
        emoji: "🔨",
        name: "Martelo",
        category: EmojiCategory::Objects,
        keywords: "martelo ferramenta construir obra reforma hammer tool build construct",
    },
    EmojiEntry {
        emoji: "✂",
        name: "Tesoura",
        category: EmojiCategory::Objects,
        keywords: "tesoura cortar recortar edição papelaria scissors cut trim edit",
    },
    EmojiEntry {
        emoji: "📏",
        name: "Régua",
        category: EmojiCategory::Objects,
        keywords: "régua medida tamanho engenharia desenho ruler measure length",
    },
    EmojiEntry {
        emoji: "📦",
        name: "Caixa",
        category: EmojiCategory::Objects,
        keywords: "caixa pacote entrega envio produto release package box delivery shipping",
    },
    EmojiEntry {
        emoji: "📬",
        name: "Caixa de correio",
        category: EmojiCategory::Objects,
        keywords: "correio carta mensagem entrada email mailbox mail inbox letter",
    },
    EmojiEntry {
        emoji: "✉",
        name: "Envelope",
        category: EmojiCategory::Objects,
        keywords: "envelope carta email mensagem contato envelope letter email message",
    },
    EmojiEntry {
        emoji: "🔔",
        name: "Sino",
        category: EmojiCategory::Objects,
        keywords: "sino notificação alerta lembrete aviso bell notification alert reminder",
    },
    EmojiEntry {
        emoji: "📢",
        name: "Megafone",
        category: EmojiCategory::Objects,
        keywords: "megafone anúncio aviso comunicação novidade loudspeaker announcement news",
    },
    EmojiEntry {
        emoji: "💰",
        name: "Saco de dinheiro",
        category: EmojiCategory::Objects,
        keywords: "dinheiro finanças orçamento riqueza economia salário money bag finance budget wealth",
    },
    EmojiEntry {
        emoji: "💳",
        name: "Cartão",
        category: EmojiCategory::Objects,
        keywords: "cartão crédito banco pagamento compra finanças credit card bank payment",
    },
    EmojiEntry {
        emoji: "💵",
        name: "Nota de dinheiro",
        category: EmojiCategory::Objects,
        keywords: "dinheiro nota dólar pagamento finanças cash dollar bill money",
    },
    EmojiEntry {
        emoji: "🎬",
        name: "Claquete",
        category: EmojiCategory::Objects,
        keywords: "claquete cinema filme vídeo produção série clapper movie film video",
    },
    EmojiEntry {
        emoji: "🎧",
        name: "Fones de ouvido",
        category: EmojiCategory::Objects,
        keywords: "fone ouvido música podcast áudio som headphones music podcast audio",
    },
    EmojiEntry {
        emoji: "🎸",
        name: "Guitarra",
        category: EmojiCategory::Objects,
        keywords: "guitarra violão música banda rock instrumento guitar music instrument rock",
    },
    EmojiEntry {
        emoji: "🎮",
        name: "Controle de jogo",
        category: EmojiCategory::Objects,
        keywords: "jogo videogame controle diversão gamer game controller video gaming",
    },
    EmojiEntry {
        emoji: "🎲",
        name: "Dado",
        category: EmojiCategory::Objects,
        keywords: "dado jogo sorte aleatório rpg tabuleiro dice game random luck",
    },
    EmojiEntry {
        emoji: "⚽",
        name: "Bola de futebol",
        category: EmojiCategory::Objects,
        keywords: "futebol bola esporte jogo gol soccer football ball sport",
    },
    EmojiEntry {
        emoji: "🏀",
        name: "Basquete",
        category: EmojiCategory::Objects,
        keywords: "basquete bola esporte quadra jogo basketball ball sport",
    },
    EmojiEntry {
        emoji: "✈",
        name: "Avião",
        category: EmojiCategory::Objects,
        keywords: "avião viagem voo férias aeroporto transporte airplane flight travel trip",
    },
    EmojiEntry {
        emoji: "🚗",
        name: "Carro",
        category: EmojiCategory::Objects,
        keywords: "carro automóvel estrada viagem transporte car auto drive travel",
    },
    EmojiEntry {
        emoji: "🚲",
        name: "Bicicleta",
        category: EmojiCategory::Objects,
        keywords: "bicicleta bike pedal esporte passeio transporte bicycle bike cycling",
    },
    EmojiEntry {
        emoji: "⛺",
        name: "Barraca",
        category: EmojiCategory::Objects,
        keywords: "barraca acampamento trilha natureza aventura tent camp camping nature",
    },
    EmojiEntry {
        emoji: "🏰",
        name: "Castelo",
        category: EmojiCategory::Objects,
        keywords: "castelo reino história fantasia palácio castle kingdom fantasy",
    },
    EmojiEntry {
        emoji: "🔭",
        name: "Telescópio",
        category: EmojiCategory::Objects,
        keywords: "telescópio astronomia estrelas espaço ciência observar telescope astronomy space stars",
    },
    EmojiEntry {
        emoji: "💊",
        name: "Pílula",
        category: EmojiCategory::Objects,
        keywords: "pílula remédio saúde médico farmácia cápsula pill medicine health pharmacy",
    },
    EmojiEntry {
        emoji: "🎈",
        name: "Balão",
        category: EmojiCategory::Objects,
        keywords: "balão festa aniversário celebração leve balloon party birthday celebration",
    },
    EmojiEntry {
        emoji: "💣",
        name: "Bomba",
        category: EmojiCategory::Objects,
        keywords: "bomba explosão perigo alerta urgente bomb boom danger alert",
    },
    EmojiEntry {
        emoji: "🔮",
        name: "Bola de cristal",
        category: EmojiCategory::Objects,
        keywords: "bola cristal magia previsão futuro ideia crystal ball magic future",
    },
    // --- Símbolos, formas e clima ---
    EmojiEntry {
        emoji: "💖",
        name: "Coração brilhante",
        category: EmojiCategory::Symbols,
        keywords: "coração brilho amor carinho rosa sparkling heart love pink",
    },
    EmojiEntry {
        emoji: "💙",
        name: "Coração azul",
        category: EmojiCategory::Symbols,
        keywords: "coração azul amor confiança amizade blue heart love trust",
    },
    EmojiEntry {
        emoji: "💚",
        name: "Coração verde",
        category: EmojiCategory::Symbols,
        keywords: "coração verde amor natureza saúde green heart love nature",
    },
    EmojiEntry {
        emoji: "💛",
        name: "Coração amarelo",
        category: EmojiCategory::Symbols,
        keywords: "coração amarelo amor amizade alegria yellow heart love friendship",
    },
    EmojiEntry {
        emoji: "💜",
        name: "Coração roxo",
        category: EmojiCategory::Symbols,
        keywords: "coração roxo amor carinho purple heart love",
    },
    EmojiEntry {
        emoji: "💔",
        name: "Coração partido",
        category: EmojiCategory::Symbols,
        keywords: "coração partido triste fim término broken heart sad breakup",
    },
    EmojiEntry {
        emoji: "✨",
        name: "Brilhos",
        category: EmojiCategory::Symbols,
        keywords: "brilho brilhos mágica novo destaque limpo estrelas sparkles magic shine new clean",
    },
    EmojiEntry {
        emoji: "🌟",
        name: "Estrela brilhante",
        category: EmojiCategory::Symbols,
        keywords: "estrela brilhante destaque sucesso especial glowing star shine bright",
    },
    EmojiEntry {
        emoji: "💫",
        name: "Estrela cadente",
        category: EmojiCategory::Symbols,
        keywords: "estrela cadente brilho tontura magia dizzy star shooting magic",
    },
    EmojiEntry {
        emoji: "⚡",
        name: "Raio",
        category: EmojiCategory::Symbols,
        keywords: "raio energia rápido relâmpago elétrico força zap lightning bolt fast energy electric",
    },
    EmojiEntry {
        emoji: "💥",
        name: "Explosão",
        category: EmojiCategory::Symbols,
        keywords: "explosão impacto colisão boom energia boom collision explosion impact",
    },
    EmojiEntry {
        emoji: "☁",
        name: "Nuvem",
        category: EmojiCategory::Symbols,
        keywords: "nuvem céu clima tempo backup cloud sky weather",
    },
    EmojiEntry {
        emoji: "⛅",
        name: "Sol entre nuvens",
        category: EmojiCategory::Symbols,
        keywords: "sol nuvem clima tempo nublado partly sunny cloud weather",
    },
    EmojiEntry {
        emoji: "☔",
        name: "Chuva",
        category: EmojiCategory::Symbols,
        keywords: "chuva guarda-chuva clima água gotas rain umbrella weather drops",
    },
    EmojiEntry {
        emoji: "❄",
        name: "Floco de neve",
        category: EmojiCategory::Symbols,
        keywords: "neve floco frio inverno gelo congelado snowflake snow cold winter ice",
    },
    EmojiEntry {
        emoji: "💧",
        name: "Gota",
        category: EmojiCategory::Symbols,
        keywords: "gota água líquido suor chuva droplet water drop liquid",
    },
    EmojiEntry {
        emoji: "⚠",
        name: "Aviso",
        category: EmojiCategory::Symbols,
        keywords: "aviso alerta atenção perigo cuidado warning alert caution danger",
    },
    EmojiEntry {
        emoji: "❗",
        name: "Exclamação",
        category: EmojiCategory::Symbols,
        keywords: "exclamação importante urgente atenção alerta exclamation important urgent alert",
    },
    EmojiEntry {
        emoji: "❓",
        name: "Interrogação",
        category: EmojiCategory::Symbols,
        keywords: "interrogação pergunta dúvida ajuda faq question doubt help",
    },
    EmojiEntry {
        emoji: "❌",
        name: "Erro",
        category: EmojiCategory::Symbols,
        keywords: "erro cancelar errado fechar não x cross mark error cancel wrong no",
    },
    EmojiEntry {
        emoji: "🚫",
        name: "Proibido",
        category: EmojiCategory::Symbols,
        keywords: "proibido bloqueado não impedir vetado prohibited forbidden blocked no",
    },
    EmojiEntry {
        emoji: "♻",
        name: "Reciclagem",
        category: EmojiCategory::Symbols,
        keywords: "reciclagem sustentável ciclo renovar verde recycle green eco renew",
    },
    EmojiEntry {
        emoji: "🔄",
        name: "Atualizar",
        category: EmojiCategory::Symbols,
        keywords: "atualizar sincronizar recarregar repetir ciclo refresh sync reload repeat",
    },
    EmojiEntry {
        emoji: "🔗",
        name: "Link",
        category: EmojiCategory::Symbols,
        keywords: "link corrente conexão url referência vínculo link chain url connection",
    },
    EmojiEntry {
        emoji: "💯",
        name: "Cem pontos",
        category: EmojiCategory::Symbols,
        keywords: "cem perfeito nota máxima excelente pontuação hundred 100 perfect score",
    },
    EmojiEntry {
        emoji: "💭",
        name: "Balão de pensamento",
        category: EmojiCategory::Symbols,
        keywords: "pensamento ideia reflexão sonho mente thought bubble idea dream",
    },
    EmojiEntry {
        emoji: "💤",
        name: "Sono",
        category: EmojiCategory::Symbols,
        keywords: "sono zzz dormir pausa descanso inativo zzz sleep rest idle",
    },
    EmojiEntry {
        emoji: "🎶",
        name: "Notas musicais",
        category: EmojiCategory::Symbols,
        keywords: "música notas melodia som playlist cantar notes music melody song",
    },
    EmojiEntry {
        emoji: "🚩",
        name: "Bandeira",
        category: EmojiCategory::Symbols,
        keywords: "bandeira marco alerta sinalização meta flag milestone alert marker",
    },
    EmojiEntry {
        emoji: "🏁",
        name: "Bandeira quadriculada",
        category: EmojiCategory::Symbols,
        keywords: "chegada corrida meta fim conclusão checkered flag finish race goal",
    },
    EmojiEntry {
        emoji: "🔴",
        name: "Círculo vermelho",
        category: EmojiCategory::Symbols,
        keywords: "círculo bola vermelho ponto status urgente red circle dot status",
    },
    EmojiEntry {
        emoji: "🔵",
        name: "Círculo azul",
        category: EmojiCategory::Symbols,
        keywords: "círculo bola azul ponto status informação blue circle dot status",
    },
    EmojiEntry {
        emoji: "⚪",
        name: "Círculo branco",
        category: EmojiCategory::Symbols,
        keywords: "círculo bola branco ponto neutro white circle dot",
    },
    EmojiEntry {
        emoji: "⚫",
        name: "Círculo preto",
        category: EmojiCategory::Symbols,
        keywords: "círculo bola preto ponto escuro black circle dot",
    },
    EmojiEntry {
        emoji: "🔶",
        name: "Losango laranja",
        category: EmojiCategory::Symbols,
        keywords: "losango diamante laranja forma marcador orange diamond shape",
    },
    EmojiEntry {
        emoji: "🔷",
        name: "Losango azul",
        category: EmojiCategory::Symbols,
        keywords: "losango diamante azul forma marcador blue diamond shape",
    },
    EmojiEntry {
        emoji: "➕",
        name: "Mais",
        category: EmojiCategory::Symbols,
        keywords: "mais adicionar soma positivo novo plus add positive",
    },
    EmojiEntry {
        emoji: "➖",
        name: "Menos",
        category: EmojiCategory::Symbols,
        keywords: "menos remover subtrair negativo minus remove subtract",
    },
    EmojiEntry {
        emoji: "➡",
        name: "Seta para direita",
        category: EmojiCategory::Symbols,
        keywords: "seta direita próximo avançar ir arrow right next forward",
    },
    EmojiEntry {
        emoji: "⚓",
        name: "Âncora",
        category: EmojiCategory::Symbols,
        keywords: "âncora mar porto estável referência navio anchor sea port stable",
    },
];

/// Retorna os itens do catálogo que pertencem à categoria escolhida.
///
/// Para `EmojiCategory::All`, devolve o catálogo completo (começando pelos 48
/// emojis padrão). Para `EmojiCategory::Recents`, devolve vazio — os recentes
/// vivem em `App::recent_emojis` porque podem incluir emojis colados fora da lista.
pub fn by_category(category: EmojiCategory) -> Vec<&'static EmojiEntry> {
    match category {
        EmojiCategory::All => CATALOG.iter().collect(),
        EmojiCategory::Recents => Vec::new(),
        other => CATALOG
            .iter()
            .filter(|entry| entry.category == other)
            .collect(),
    }
}

/// Filtra o catálogo por busca instantânea, ignorando maiúsculas e acentos (PT e EN).
///
/// Suporta múltiplos termos separados por espaço e colagem direta do próprio emoji.
pub fn search(query: &str) -> Vec<&'static EmojiEntry> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return CATALOG.iter().collect();
    }
    let folded_query = fold(trimmed);
    let tokens: Vec<&str> = folded_query.split_whitespace().collect();
    if tokens.is_empty() {
        return CATALOG.iter().collect();
    }

    let mut ranked: Vec<(u8, usize, &'static EmojiEntry)> = Vec::new();
    for (index, entry) in CATALOG.iter().enumerate() {
        if entry.emoji == trimmed {
            ranked.push((0, index, entry));
            continue;
        }
        let haystack_name = fold(entry.name);
        let haystack_kw = fold(entry.keywords);
        let matches_all = tokens.iter().all(|token| {
            entry.emoji.contains(token)
                || haystack_name.contains(token)
                || haystack_kw.contains(token)
        });
        if !matches_all {
            continue;
        }
        let rank = if haystack_name == folded_query
            || haystack_kw
                .split_whitespace()
                .any(|word| word == folded_query)
        {
            1
        } else if haystack_name.starts_with(&folded_query)
            || haystack_kw
                .split_whitespace()
                .any(|word| word.starts_with(&folded_query))
        {
            2
        } else {
            3
        };
        ranked.push((rank, index, entry));
    }
    ranked.sort_by_key(|(rank, index, _)| (*rank, *index));
    ranked.into_iter().map(|(_, _, entry)| entry).collect()
}

/// Normaliza o texto colado/digitado no campo livre de ícone.
///
/// Remove espaços nas pontas e quebras de linha; retorna `None` se ficar vazio.
pub fn normalize_custom_emoji(input: &str) -> Option<String> {
    let cleaned: String = input
        .chars()
        .filter(|ch| !matches!(ch, '\r' | '\n' | '\t'))
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

/// Lê a lista de emojis recentes gravada em `settings.recent_emojis`.
pub fn parse_recent_emojis(raw: Option<&str>) -> Vec<String> {
    let Some(raw) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return Vec::new();
    };
    let Ok(items) = serde_json::from_str::<Vec<String>>(raw) else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(RECENT_EMOJIS_LIMIT);
    for item in items {
        let Some(clean) = normalize_custom_emoji(&item) else {
            continue;
        };
        if !out.contains(&clean) {
            out.push(clean);
            if out.len() >= RECENT_EMOJIS_LIMIT {
                break;
            }
        }
    }
    out
}

/// Serializa até `RECENT_EMOJIS_LIMIT` emojis recentes para a tabela `settings`.
pub fn format_recent_emojis(recents: &[String]) -> String {
    let limit = recents.len().min(RECENT_EMOJIS_LIMIT);
    serde_json::to_string(&recents[..limit]).unwrap_or_else(|_| "[]".to_owned())
}

/// Move `icon` para o topo da lista de recentes, mantendo no máximo 8 itens únicos.
///
/// Retorna `false` se `icon` for vazio (ex.: ao remover o ícone da página).
pub fn record_recent_emoji(recents: &mut Vec<String>, icon: &str) -> bool {
    let Some(clean) = normalize_custom_emoji(icon) else {
        return false;
    };
    recents.retain(|existing| existing != &clean);
    recents.insert(0, clean);
    recents.truncate(RECENT_EMOJIS_LIMIT);
    true
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    const ORIGINAL_48: [&str; 48] = [
        "📄", "📝", "📚", "📌", "💡", "✅", "🚀", "⭐", "❤", "🎯", "🔥", "🌱", "🍀", "🌊", "☀",
        "🌙", "🎵", "🎨", "🏠", "💼", "📅", "🔍", "⚙", "🔒", "🎁", "🐱", "🐶", "🍎", "🌻", "🌈",
        "☕", "🏆", "📷", "✏", "📎", "🎓", "💬", "🛠", "🎉", "🔬", "🔖", "📊", "🗺", "⏰", "💎",
        "🐢", "🌸", "🍕",
    ];

    #[test]
    fn catalog_starts_with_original_48_and_has_no_duplicates() {
        assert!(CATALOG.len() >= 200);
        let first_48: Vec<&str> = CATALOG.iter().take(48).map(|entry| entry.emoji).collect();
        assert_eq!(first_48, ORIGINAL_48);

        let mut seen = HashSet::new();
        for entry in CATALOG {
            assert!(
                seen.insert(entry.emoji),
                "emoji duplicado no catálogo: {}",
                entry.emoji
            );
            assert!(
                !matches!(entry.category, EmojiCategory::All | EmojiCategory::Recents),
                "entrada {} precisa pertencer a uma categoria concreta",
                entry.emoji
            );
            assert!(!entry.name.is_empty());
            assert!(!entry.keywords.is_empty());
        }
    }

    #[test]
    fn every_content_category_is_populated() {
        for cat in [
            EmojiCategory::Smileys,
            EmojiCategory::Animals,
            EmojiCategory::Food,
            EmojiCategory::Objects,
            EmojiCategory::Symbols,
        ] {
            let items = by_category(cat);
            assert!(
                items.len() >= 30,
                "categoria {:?} ficou pequena ({})",
                cat,
                items.len()
            );
            assert!(items.iter().all(|entry| entry.category == cat));
        }
        assert_eq!(by_category(EmojiCategory::All).len(), CATALOG.len());
        assert!(by_category(EmojiCategory::Recents).is_empty());
    }

    #[test]
    fn search_matches_pt_en_case_and_accent_insensitive() {
        let fogo = search("fogo");
        assert_eq!(fogo.first().map(|e| e.emoji), Some("🔥"));

        let coracao = search("coracao");
        assert!(
            coracao.iter().any(|e| e.emoji == "❤"),
            "busca 'coracao' deveria incluir ❤"
        );
        assert_eq!(coracao.first().map(|e| e.emoji), Some("❤"));

        let coracao_accent = search("CORAÇÃO");
        assert_eq!(coracao_accent.first().map(|e| e.emoji), Some("❤"));

        let flower = search("flower");
        assert!(
            flower.iter().any(|e| e.emoji == "🌸"),
            "busca 'flower' deveria incluir 🌸"
        );

        let direct = search("🔥");
        assert_eq!(direct.first().map(|e| e.emoji), Some("🔥"));

        let empty = search("   ");
        assert_eq!(empty.len(), CATALOG.len());

        assert!(search("nao-existe-xyz-999").is_empty());
    }

    #[test]
    fn recents_deduplicate_cap_at_eight_and_round_trip() {
        let mut recents = Vec::new();
        assert!(!record_recent_emoji(&mut recents, ""));
        assert!(!record_recent_emoji(&mut recents, "   "));
        assert!(recents.is_empty());

        for emoji in ["🔥", "❤", "🌸", "🫠", "🚀", "⭐", "🍕", "🐱", "🐶"] {
            assert!(record_recent_emoji(&mut recents, emoji));
        }
        assert_eq!(recents.len(), RECENT_EMOJIS_LIMIT);
        assert_eq!(recents[0], "🐶");
        assert!(!recents.contains(&"🔥".to_owned()));

        // Repetir item existente move para o topo sem duplicar.
        assert!(record_recent_emoji(&mut recents, "🫠"));
        assert_eq!(recents.len(), RECENT_EMOJIS_LIMIT);
        assert_eq!(recents[0], "🫠");
        assert_eq!(recents.iter().filter(|e| e.as_str() == "🫠").count(), 1);

        let encoded = format_recent_emojis(&recents);
        assert_eq!(parse_recent_emojis(Some(&encoded)), recents);
        assert!(parse_recent_emojis(None).is_empty());
        assert!(parse_recent_emojis(Some("")).is_empty());
        assert!(parse_recent_emojis(Some("invalido")).is_empty());
    }

    #[test]
    fn normalize_custom_emoji_handles_pasted_unicode() {
        assert_eq!(normalize_custom_emoji(" 🫠 \n"), Some("🫠".to_owned()));
        assert_eq!(normalize_custom_emoji("👨‍👩‍👧‍👦"), Some("👨‍👩‍👧‍👦".to_owned()));
        assert_eq!(normalize_custom_emoji("   \t\n "), None);
    }
}
